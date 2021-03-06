//std
use std;
use std::sync::Arc;
// pbrt
use core::interaction::SurfaceInteraction;
use core::material::{Material, TransportMode};
use core::microfacet::TrowbridgeReitzDistribution;
use core::paramset::TextureParams;
use core::pbrt::{Float, Spectrum};
use core::reflection::{Bsdf, Bxdf, FresnelDielectric, LambertianReflection, MicrofacetReflection,
                       SpecularReflection, SpecularTransmission};
use core::texture::Texture;

// see uber.h

pub struct UberMaterial {
    pub kd: Arc<Texture<Spectrum> + Sync + Send>, // default: 0.25
    pub ks: Arc<Texture<Spectrum> + Sync + Send>, // default: 0.25
    pub kr: Arc<Texture<Spectrum> + Sync + Send>, // default: 0.0
    pub kt: Arc<Texture<Spectrum> + Sync + Send>, // default: 0.0
    pub roughness: Arc<Texture<Float> + Sync + Send>, // default: 0.1
    pub u_roughness: Option<Arc<Texture<Float> + Sync + Send>>,
    pub v_roughness: Option<Arc<Texture<Float> + Sync + Send>>,
    pub eta: Arc<Texture<Float> + Sync + Send>, // default: 1.5
    pub opacity: Arc<Texture<Spectrum> + Sync + Send>, // default: 1.0
    // TODO: bump_map
    pub remap_roughness: bool,
}

impl UberMaterial {
    pub fn new(
        kd: Arc<Texture<Spectrum> + Sync + Send>,
        ks: Arc<Texture<Spectrum> + Sync + Send>,
        kr: Arc<Texture<Spectrum> + Sync + Send>,
        kt: Arc<Texture<Spectrum> + Sync + Send>,
        roughness: Arc<Texture<Float> + Sync + Send>,
        u_roughness: Option<Arc<Texture<Float> + Sync + Send>>,
        v_roughness: Option<Arc<Texture<Float> + Sync + Send>>,
        eta: Arc<Texture<Float> + Send + Sync>,
        opacity: Arc<Texture<Spectrum> + Sync + Send>,
        remap_roughness: bool,
    ) -> Self {
        UberMaterial {
            kd: kd,
            ks: ks,
            kr: kr,
            kt: kt,
            roughness: roughness,
            u_roughness: u_roughness,
            v_roughness: v_roughness,
            eta: eta,
            opacity: opacity,
            remap_roughness: remap_roughness,
        }
    }
    pub fn create(mp: &mut TextureParams) -> Arc<Material + Send + Sync> {
        let kd: Arc<Texture<Spectrum> + Sync + Send> =
            mp.get_spectrum_texture(String::from("Kd"), Spectrum::new(0.25));
        let ks: Arc<Texture<Spectrum> + Sync + Send> =
            mp.get_spectrum_texture(String::from("Ks"), Spectrum::new(0.25));
        let kr: Arc<Texture<Spectrum> + Sync + Send> =
            mp.get_spectrum_texture(String::from("Kr"), Spectrum::new(0.0));
        let kt: Arc<Texture<Spectrum> + Sync + Send> =
            mp.get_spectrum_texture(String::from("Kt"), Spectrum::new(0.0));
        let roughness: Arc<Texture<Float> + Send + Sync> =
            mp.get_float_texture(String::from("roughness"), 0.1 as Float);
        let u_roughness: Option<Arc<Texture<Float> + Send + Sync>> =
            mp.get_float_texture_or_null(String::from("uroughness"));
        let v_roughness: Option<Arc<Texture<Float> + Send + Sync>> =
            mp.get_float_texture_or_null(String::from("vroughness"));
        let opacity: Arc<Texture<Spectrum> + Send + Sync> =
            mp.get_spectrum_texture(String::from("opacity"), Spectrum::new(1.0));
        // TODO: std::shared_ptr<Texture<Float>> bumpMap = mp.GetFloatTextureOrNull("bumpmap");
        let remap_roughness: bool = mp.find_bool(String::from("remaproughness"), true);
        let eta_option: Option<Arc<Texture<Float> + Send + Sync>> =
            mp.get_float_texture_or_null(String::from("eta"));
        if let Some(ref eta) = eta_option {
            Arc::new(UberMaterial::new(
                kd,
                ks,
                kr,
                kt,
                roughness,
                u_roughness,
                v_roughness,
                eta.clone(),
                opacity,
                remap_roughness,
            ))
        } else {
            let eta: Arc<Texture<Float> + Send + Sync> =
                mp.get_float_texture(String::from("eta"), 1.5 as Float);
            Arc::new(UberMaterial::new(
                kd,
                ks,
                kr,
                kt,
                roughness,
                u_roughness,
                v_roughness,
                eta,
                opacity,
                remap_roughness,
            ))
        }
    }
    pub fn bsdf(&self, si: &SurfaceInteraction, mode: TransportMode) -> Bsdf {
        let mut bxdfs: Vec<Arc<Bxdf + Send + Sync>> = Vec::new();
        let e: Float = self.eta.evaluate(si);
        let op: Spectrum = self.opacity
            .evaluate(si)
            .clamp(0.0 as Float, std::f32::INFINITY as Float);
        let t: Spectrum =
            (Spectrum::new(1.0) - op).clamp(0.0 as Float, std::f32::INFINITY as Float);
        if !t.is_black() {
            bxdfs.push(Arc::new(SpecularTransmission::new(
                t,
                1.0,
                1.0,
                mode.clone(),
            )));
        }
        let kd: Spectrum = op
            * self.kd
                .evaluate(si)
                .clamp(0.0 as Float, std::f32::INFINITY as Float);
        if !kd.is_black() {
            bxdfs.push(Arc::new(LambertianReflection::new(kd)));
        }
        let ks: Spectrum = op
            * self.ks
                .evaluate(si)
                .clamp(0.0 as Float, std::f32::INFINITY as Float);
        if !ks.is_black() {
            let fresnel = Arc::new(FresnelDielectric {
                eta_i: 1.0,
                eta_t: e,
            });
            let mut u_rough: Float;
            if let Some(ref u_roughness) = self.u_roughness {
                u_rough = u_roughness.evaluate(si);
            } else {
                u_rough = self.roughness.evaluate(si);
            }
            let mut v_rough: Float;
            if let Some(ref v_roughness) = self.v_roughness {
                v_rough = v_roughness.evaluate(si);
            } else {
                v_rough = self.roughness.evaluate(si);
            }
            if self.remap_roughness {
                u_rough = TrowbridgeReitzDistribution::roughness_to_alpha(u_rough);
                v_rough = TrowbridgeReitzDistribution::roughness_to_alpha(v_rough);
            }
            let distrib: Option<TrowbridgeReitzDistribution> =
                Some(TrowbridgeReitzDistribution::new(u_rough, v_rough, true));
            bxdfs.push(Arc::new(MicrofacetReflection::new(ks, distrib, fresnel)));
        }
        let kr: Spectrum = op
            * self.kr
                .evaluate(si)
                .clamp(0.0 as Float, std::f32::INFINITY as Float);
        if !kr.is_black() {
            let fresnel = Arc::new(FresnelDielectric {
                eta_i: 1.0,
                eta_t: e,
            });
            bxdfs.push(Arc::new(SpecularReflection::new(kr, fresnel)));
        }
        let kt: Spectrum = op
            * self.kt
                .evaluate(si)
                .clamp(0.0 as Float, std::f32::INFINITY as Float);
        if !kt.is_black() {
            bxdfs.push(Arc::new(SpecularTransmission::new(
                kt,
                1.0,
                e,
                mode.clone(),
            )));
        }
        if !t.is_black() {
            Bsdf::new(si, 1.0, bxdfs)
        } else {
            Bsdf::new(si, e, bxdfs)
        }
    }
}

impl Material for UberMaterial {
    fn compute_scattering_functions(
        &self,
        si: &mut SurfaceInteraction,
        // arena: &mut Arena,
        mode: TransportMode,
        _allow_multiple_lobes: bool,
    ) {
        si.bsdf = Some(Arc::new(self.bsdf(si, mode)));
    }
}
