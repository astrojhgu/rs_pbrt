// pbrt
use core::camera::CameraSample;
use core::pbrt::Float;
use geometry::{Point2f, Point2i};

// see sampler.h

pub trait Sampler {
    fn start_pixel(&mut self, p: Point2i);
    fn get_1d(&mut self) -> Float;
    fn get_2d(&mut self) -> Point2f;
    fn request_2d_array(&mut self, n: i32);
    fn round_count(&self, count: i32) -> i32;
    fn get_2d_array(&mut self, n: i32) -> Vec<Point2f>;
    fn start_next_sample(&mut self) -> bool;
    fn get_camera_sample(&mut self, p_raster: Point2i) -> CameraSample;
    fn reseed(&mut self, seed: u64);
    fn box_clone(&self) -> Box<Sampler + Send + Sync>;
    fn get_current_sample_number(&self) -> i64;
    fn get_samples_per_pixel(&self) -> i64;
}

impl Clone for Box<Sampler + Send + Sync> {
    fn clone(&self) -> Box<Sampler + Send + Sync> {
        self.box_clone()
    }
}