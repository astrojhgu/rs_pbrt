// std
use std::f32::consts::PI;
use std::sync::Arc;
// pbrt
use core::efloat::EFloat;
use core::efloat::quadratic_efloat;
use core::geometry::{Bounds3f, Normal3f, Point2f, Point3f, Ray, Vector3f};
use core::geometry::{nrm_dot_nrm, nrm_normalize, bnd3_expand, bnd3_union_bnd3, nrm_abs_dot_vec3,
                     pnt3_distance, pnt3_distance_squared, pnt3_lerp, pnt3_offset_ray_origin,
                     spherical_direction_vec3, vec3_coordinate_system, vec3_cross_vec3,
                     vec3_dot_vec3, vec3_normalize};
use core::interaction::{Interaction, InteractionCommon, SurfaceInteraction};
use core::material::Material;
use core::pbrt::Float;
use core::pbrt::{clamp_t, gamma, lerp, radians};
use core::sampling::{uniform_cone_pdf, uniform_sample_sphere};
use core::shape::Shape;
use core::transform::Transform;

// see curve.h

#[derive(Debug, Clone, PartialEq)]
pub enum CurveType {
    Flat,
    Cylinder,
    Ribbon,
}

#[derive(Clone)]
pub struct CurveCommon {
    pub curve_type: CurveType,
    pub cp_obj: [Point3f; 4],
    pub width: [Float; 2],
    pub n: [Normal3f; 2],
    pub normal_angle: Float,
    pub inv_sin_normal_angle: Float,
}

impl CurveCommon {
    pub fn new(
        c: &[Point3f; 4],
        width0: Float,
        width1: Float,
        curve_type: CurveType,
        norm: Option<[Normal3f; 2]>,
    ) -> Self {
        if let Some(norm) = norm {
            let n0: Normal3f = nrm_normalize(norm[0]);
            let n1: Normal3f = nrm_normalize(norm[1]);
            let normal_angle: Float =
                clamp_t(nrm_dot_nrm(n0, n1), 0.0 as Float, 1.0 as Float).acos();
            let inv_sin_normal_angle: Float = 1.0 as Float / normal_angle.sin();
            CurveCommon {
                curve_type: curve_type,
                cp_obj: [c[0], c[1], c[2], c[3]],
                width: [width0, width1],
                n: [n0, n1],
                normal_angle: normal_angle,
                inv_sin_normal_angle: inv_sin_normal_angle,
            }
        } else {
            CurveCommon {
                curve_type: curve_type,
                cp_obj: [c[0], c[1], c[2], c[3]],
                width: [width0, width1],
                n: [Normal3f::default(); 2],
                normal_angle: 0.0 as Float,
                inv_sin_normal_angle: 0.0 as Float,
            }
        }
    }
}

#[derive(Clone)]
pub struct Curve {
    pub common: Arc<CurveCommon>,
    pub u_min: Float,
    pub u_max: Float,
    // inherited from class Shape (see shape.h)
    object_to_world: Transform,
    world_to_object: Transform,
    reverse_orientation: bool,
    transform_swaps_handedness: bool,
    pub material: Option<Arc<Material + Send + Sync>>,
}

impl Curve {
    pub fn new(
        object_to_world: Transform,
        world_to_object: Transform,
        reverse_orientation: bool,
        common: Arc<CurveCommon>,
        u_min: Float,
        u_max: Float,
    ) -> Self {
        Curve {
            // Curve
            common: common,
            u_min: u_min,
            u_max: u_max,
            // Shape
            object_to_world: object_to_world,
            world_to_object: world_to_object,
            reverse_orientation: reverse_orientation,
            transform_swaps_handedness: false,
            material: None,
        }
    }
    pub fn create(
        o2w: Transform,
        w2o: Transform,
        reverse_orientation: bool,
        c: &[Point3f; 4],
        w0: Float,
        w1: Float,
        curve_type: CurveType,
        norm: Option<[Normal3f; 2]>,
        split_depth: i32,
    ) -> Vec<Arc<Shape + Send + Sync>> {
        let common: Arc<CurveCommon> = Arc::new(CurveCommon::new(c, w0, w1, curve_type, norm));
        let n_segments: usize = 1_usize << split_depth;
        let mut segments: Vec<Arc<Shape + Send + Sync>> = Vec::with_capacity(n_segments);
        for i in 0..n_segments {
            let u_min: Float = i as Float / n_segments as Float;
            let u_max: Float = (i + 1) as Float / n_segments as Float;
            // segments.push_back(std::make_shared<Curve>(o2w, w2o, reverseOrientation,
            //                                            common, u_min, u_max));
            let curve: Arc<Curve> = Arc::new(Curve::new(
                o2w,
                w2o,
                reverse_orientation,
                common.clone(),
                u_min,
                u_max,
            ));
            segments.push(curve.clone());
            // TODO: ++nSplitCurves;
        }
        // TODO: curveBytes += sizeof(CurveCommon) + n_segments * sizeof(Curve);
        segments
    }
}

impl Shape for Curve {
    fn object_bound(&self) -> Bounds3f {
        // compute object-space control points for curve segment, _cpObj_
        let mut cp_obj: [Point3f; 4] = [Point3f::default(); 4];
        cp_obj[0] = blossom_bezier(&self.common.cp_obj, self.u_min, self.u_min, self.u_min);
        cp_obj[1] = blossom_bezier(&self.common.cp_obj, self.u_min, self.u_min, self.u_max);
        cp_obj[2] = blossom_bezier(&self.common.cp_obj, self.u_min, self.u_max, self.u_max);
        cp_obj[3] = blossom_bezier(&self.common.cp_obj, self.u_max, self.u_max, self.u_max);
        let b: Bounds3f = bnd3_union_bnd3(
            Bounds3f::new(cp_obj[0], cp_obj[1]),
            Bounds3f::new(cp_obj[2], cp_obj[3]),
        );
        let width: [Float; 2] = [
            lerp(self.u_min, self.common.width[0], self.common.width[1]),
            lerp(self.u_max, self.common.width[0], self.common.width[1]),
        ];
        bnd3_expand(b, width[0].max(width[1]) * 0.5 as Float)
    }
    fn world_bound(&self) -> Bounds3f {
        // in C++: Bounds3f Shape::WorldBound() const { return (*ObjectToWorld)(ObjectBound()); }
        self.object_to_world.transform_bounds(self.object_bound())
    }
    fn intersect(&self, r: &Ray) -> Option<(SurfaceInteraction, Float)> {
        // TODO
        None
    }
    fn intersect_p(&self, r: &Ray) -> bool {
        // TODO
        false
    }
    fn get_reverse_orientation(&self) -> bool {
        self.reverse_orientation
    }
    fn get_transform_swaps_handedness(&self) -> bool {
        self.transform_swaps_handedness
    }
    fn area(&self) -> Float {
        // compute object-space control points for curve segment, _cpObj_
        let mut cp_obj: [Point3f; 4] = [Point3f::default(); 4];
        cp_obj[0] = blossom_bezier(&self.common.cp_obj, self.u_min, self.u_min, self.u_min);
        cp_obj[1] = blossom_bezier(&self.common.cp_obj, self.u_min, self.u_min, self.u_max);
        cp_obj[2] = blossom_bezier(&self.common.cp_obj, self.u_min, self.u_max, self.u_max);
        cp_obj[3] = blossom_bezier(&self.common.cp_obj, self.u_max, self.u_max, self.u_max);
        let width0: Float = lerp(self.u_min, self.common.width[0], self.common.width[1]);
        let width1: Float = lerp(self.u_max, self.common.width[0], self.common.width[1]);
        let avg_width: Float = (width0 + width1) * 0.5 as Float;
        let mut approx_length: Float = 0.0 as Float;
        for i in 0..3 {
            approx_length += pnt3_distance(cp_obj[i], cp_obj[i + 1]);
        }
        approx_length * avg_width
    }
    fn sample(&self, u: Point2f, pdf: &mut Float) -> InteractionCommon {
        println!("FATAL: Curve::sample not implemented.");
        InteractionCommon::default()
    }
    fn sample_with_ref_point(
        &self,
        iref: &InteractionCommon,
        u: Point2f,
        pdf: &mut Float,
    ) -> InteractionCommon {
        let intr: InteractionCommon = self.sample(u, pdf);
        let mut wi: Vector3f = intr.p - iref.p;
        if wi.length_squared() == 0.0 as Float {
            *pdf = 0.0 as Float;
        } else {
            wi = vec3_normalize(wi);
            // convert from area measure, as returned by the Sample()
            // call above, to solid angle measure.
            *pdf *= pnt3_distance_squared(iref.p, intr.p) / nrm_abs_dot_vec3(intr.n, -wi);
            if (*pdf).is_infinite() {
                *pdf = 0.0 as Float;
            }
        }
        intr
    }
    fn pdf(&self, iref: &Interaction, wi: Vector3f) -> Float {
        // TODO
        0.0 as Float
    }
}

// Curve Utility Functions

fn blossom_bezier(p: &[Point3f; 4], u0: Float, u1: Float, u2: Float) -> Point3f {
    let a: [Point3f; 3] = [
        pnt3_lerp(u0, p[0], p[1]),
        pnt3_lerp(u0, p[1], p[2]),
        pnt3_lerp(u0, p[2], p[3]),
    ];
    let b: [Point3f; 2] = [pnt3_lerp(u1, a[0], a[1]), pnt3_lerp(u1, a[1], a[2])];
    pnt3_lerp(u2, b[0], b[1])
}