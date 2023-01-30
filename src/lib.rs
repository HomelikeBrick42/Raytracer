use rand::Rng;
use thallium::math::{Vector2, Vector3, Zero};

#[derive(Clone, Copy)]
pub struct Ray {
    pub origin: Vector3<f32>,
    pub direction: Vector3<f32>,
}

#[derive(Clone, Copy)]
pub struct Material {
    pub diffuse_color: Vector3<f32>,
    pub emit_color: Vector3<f32>,
    pub reflectiveness: f32,
}

#[derive(Clone, Copy)]
pub struct Hit {
    pub position: Vector3<f32>,
    pub normal: Vector3<f32>,
    pub distance: f32,
}

#[derive(Clone, Copy)]
pub enum Object {
    Sphere {
        center: Vector3<f32>,
        radius: f32,
        material: Material,
    },
    Plane {
        normal: Vector3<f32>,
        distance_along_normal: f32,
        material: Material,
    },
}

impl Object {
    pub fn get_material(&self) -> &Material {
        match self {
            Object::Sphere { material, .. } | Object::Plane { material, .. } => material,
        }
    }

    pub fn intersect(&self, ray: Ray) -> Option<Hit> {
        match *self {
            Object::Sphere {
                center,
                radius,
                material: _,
            } => {
                let oc = ray.origin - center;
                let a = ray.direction.sqr_length();
                let half_b = oc.dot(ray.direction);
                let c = oc.sqr_length() - radius * radius;
                let discriminant = half_b * half_b - a * c;

                if discriminant < 0.0 {
                    return None;
                }

                let distance = (-half_b - discriminant.sqrt()) / a;
                if distance <= 0.0 {
                    return None;
                }

                let position = ray.origin + ray.direction * distance.into();
                let normal = (position - center) * (1.0 / radius).into();
                Some(Hit {
                    position,
                    normal,
                    distance,
                })
            }
            Object::Plane {
                normal,
                distance_along_normal,
                material: _,
            } => {
                let vd = normal.dot(ray.direction);
                // vd == 0.0 for double sided, vd >= 0.0 for one sided
                if vd >= 0.0 {
                    return None;
                }

                let vo = -(normal.dot(ray.origin) + distance_along_normal);
                let distance = vo / vd;
                if distance <= 0.0 {
                    return None;
                }

                let position = ray.origin + ray.direction * distance.into();
                Some(Hit {
                    position,
                    normal,
                    distance,
                })
            }
        }
    }
}

#[derive(Clone, Copy)]
pub struct Camera {
    pub position: Vector3<f32>,
    pub right: Vector3<f32>,
    pub up: Vector3<f32>,
    pub forward: Vector3<f32>,
}

impl Camera {
    pub fn get_uv(
        Vector2 { x, y }: Vector2<usize>,
        Vector2 {
            x: width,
            y: height,
        }: Vector2<usize>,
    ) -> Vector2<f32> {
        Vector2 {
            x: x as f32 / width as f32,
            y: y as f32 / height as f32,
        }
    }

    pub fn get_ray(&self, uv: Vector2<f32>, aspect: f32) -> Ray {
        Ray {
            origin: self.position,
            direction: ((self.right * ((uv.x * 2.0 - 1.0) * aspect).into())
                + (self.up * (uv.y * 2.0 - 1.0).into())
                + self.forward)
                .normalized(),
        }
    }
}

pub const SAMPLES_PER_BOUNCE: usize = 2;
pub const BOUNCES: usize = 5;
pub const DAY: bool = false;

pub fn get_closest_object(ray: Ray, objects: &[Object]) -> Option<(Hit, usize)> {
    objects
        .iter()
        .enumerate()
        .fold(None, |hit, (index, object)| {
            let new_hit = object.intersect(ray).map(|new_hit| (new_hit, index));
            hit.zip(new_hit).map_or_else(
                || hit.or(new_hit),
                |(hit, new_hit)| {
                    if hit.0.distance < new_hit.0.distance {
                        Some(hit)
                    } else {
                        Some(new_hit)
                    }
                },
            )
        })
}

pub fn ray_trace(
    ray: Ray,
    objects: &[Object],
    rng: &mut dyn rand::RngCore,
    depth: usize,
) -> Vector3<f32> {
    if depth == 0 {
        return Vector3::zero();
    }

    let hit = get_closest_object(ray, objects);

    if let Some((hit, index)) = hit {
        fn random_in_direction(
            rng: &mut dyn rand::RngCore,
            direction: Vector3<f32>,
        ) -> Vector3<f32> {
            let random = Vector3 {
                x: rng.gen::<f32>() * 2.0 - 1.0,
                y: rng.gen::<f32>() * 2.0 - 1.0,
                z: rng.gen::<f32>() * 2.0 - 1.0,
            };
            random * random.dot(direction).signum().into()
        }

        let material = objects[index].get_material();
        let direction = ray.direction.reflect(hit.normal);

        let mut in_color: Vector3<f32> = Vector3::zero();
        for _ in 0..SAMPLES_PER_BOUNCE {
            in_color += ray_trace(
                Ray {
                    origin: hit.position + hit.normal * 0.001.into(),
                    direction: random_in_direction(rng, direction)
                        * (1.0 - material.reflectiveness).into()
                        + direction * material.reflectiveness.into(),
                },
                objects,
                rng,
                depth - 1,
            );
        }
        in_color *= (1.0 / SAMPLES_PER_BOUNCE as f32).into();

        material.emit_color + material.diffuse_color * in_color
    } else {
        if DAY {
            let t = ray.direction.y * 0.5 + 0.5;
            let up_color: Vector3<f32> = (1.0, 1.0, 1.0).into();
            let down_color: Vector3<f32> = (0.5, 0.7, 1.0).into();
            up_color * (1.0 - t).into() + down_color * t.into()
        } else {
            (0.1, 0.1, 0.1).into()
        }
    }
}
