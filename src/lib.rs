use thallium::math::Vector3;

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
    pub material: Material,
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
    pub fn intersect(&self, ray: Ray) -> Option<Hit> {
        match *self {
            Object::Sphere {
                center,
                radius,
                material,
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
                    material,
                })
            }
            Object::Plane {
                normal,
                distance_along_normal,
                material,
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
                    material,
                })
            }
        }
    }
}
