use crate::math::{Vec3, rotate_euler};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mesh {
    pub vertices: Vec<Vec3>,
    pub triangles: Vec<[usize; 3]>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PrimitiveKind {
    Cube,
    Sphere,
    Cylinder,
    Cone,
    Torus,
    Wedge,
    Plane,
    Imported,
}

impl PrimitiveKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Cube => "Cube",
            Self::Sphere => "UV Sphere",
            Self::Cylinder => "Cylinder",
            Self::Cone => "Cone",
            Self::Torus => "Torus",
            Self::Wedge => "Wedge",
            Self::Plane => "Plane",
            Self::Imported => "Imported Mesh",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Transform {
    pub position: Vec3,
    pub rotation: Vec3,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            scale: Vec3::new(1.0, 1.0, 1.0),
        }
    }
}

impl Transform {
    pub fn point(&self, point: Vec3) -> Vec3 {
        rotate_euler(point.component_mul(self.scale), self.rotation) + self.position
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Object {
    pub id: u64,
    pub name: String,
    pub kind: PrimitiveKind,
    pub transform: Transform,
    pub visible: bool,
    pub mesh: Mesh,
}

impl Object {
    pub fn world_dimensions(&self) -> Vec3 {
        if self.mesh.vertices.is_empty() {
            return Vec3::ZERO;
        }
        let mut min = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
        let mut max = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
        for vertex in &self.mesh.vertices {
            let point = self.transform.point(*vertex);
            min.x = min.x.min(point.x);
            min.y = min.y.min(point.y);
            min.z = min.z.min(point.z);
            max.x = max.x.max(point.x);
            max.y = max.y.max(point.y);
            max.z = max.z.max(point.z);
        }
        max - min
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ShadingMode {
    Solid,
    Wireframe,
    XRay,
}

impl ShadingMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Solid => "SOLID",
            Self::Wireframe => "WIRE",
            Self::XRay => "X-RAY",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Camera {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub target: Vec3,
    pub perspective: bool,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            yaw: -35.0,
            pitch: 24.0,
            distance: 8.0,
            target: Vec3::ZERO,
            perspective: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    pub name: String,
    pub objects: Vec<Object>,
    pub selected: usize,
    pub camera: Camera,
    pub shading: ShadingMode,
    #[serde(default = "default_next_id")]
    next_id: u64,
}

fn default_next_id() -> u64 {
    1
}

impl Default for Scene {
    fn default() -> Self {
        let mut scene = Self {
            name: "Untitled".to_string(),
            objects: Vec::new(),
            selected: 0,
            camera: Camera::default(),
            shading: ShadingMode::Solid,
            next_id: 1,
        };
        scene.add_primitive(PrimitiveKind::Cube);
        scene
    }
}

impl Scene {
    pub fn add_primitive(&mut self, kind: PrimitiveKind) {
        let ordinal = self.objects.iter().filter(|o| o.kind == kind).count() + 1;
        let object = Object {
            id: self.next_id,
            name: format!("{} {ordinal}", kind.label()),
            kind,
            transform: Transform::default(),
            visible: true,
            mesh: primitive_mesh(kind),
        };
        self.next_id += 1;
        self.objects.push(object);
        self.selected = self.objects.len() - 1;
    }

    pub fn add_mesh(&mut self, name: String, mesh: Mesh) {
        self.objects.push(Object {
            id: self.next_id,
            name,
            kind: PrimitiveKind::Imported,
            transform: Transform::default(),
            visible: true,
            mesh,
        });
        self.next_id += 1;
        self.selected = self.objects.len() - 1;
    }

    pub fn duplicate_selected(&mut self) -> bool {
        let Some(mut duplicate) = self.selected().cloned() else {
            return false;
        };
        duplicate.id = self.next_id;
        self.next_id += 1;
        duplicate.name = format!("{} Copy", duplicate.name);
        duplicate.transform.position.x += 0.5;
        self.objects.push(duplicate);
        self.selected = self.objects.len() - 1;
        true
    }

    pub fn selected(&self) -> Option<&Object> {
        self.objects.get(self.selected)
    }

    pub fn selected_mut(&mut self) -> Option<&mut Object> {
        self.objects.get_mut(self.selected)
    }

    pub fn remove_selected(&mut self) -> Option<Object> {
        if self.objects.is_empty() {
            return None;
        }
        let object = self.objects.remove(self.selected);
        self.selected = self.selected.min(self.objects.len().saturating_sub(1));
        Some(object)
    }

    pub fn select_next(&mut self, direction: isize) {
        if self.objects.is_empty() {
            self.selected = 0;
            return;
        }
        self.selected =
            (self.selected as isize + direction).rem_euclid(self.objects.len() as isize) as usize;
    }

    pub fn world_triangles(&self) -> impl Iterator<Item = (usize, &Object, [Vec3; 3])> {
        self.objects
            .iter()
            .enumerate()
            .filter(|(_, object)| object.visible)
            .flat_map(|(object_index, object)| {
                object.mesh.triangles.iter().map(move |face| {
                    (
                        object_index,
                        object,
                        [
                            object.transform.point(object.mesh.vertices[face[0]]),
                            object.transform.point(object.mesh.vertices[face[1]]),
                            object.transform.point(object.mesh.vertices[face[2]]),
                        ],
                    )
                })
            })
    }

    pub fn repair_after_load(&mut self) {
        self.selected = self.selected.min(self.objects.len().saturating_sub(1));
        self.next_id = self.objects.iter().map(|o| o.id).max().unwrap_or(0) + 1;
        self.camera.distance = self.camera.distance.clamp(0.5, 1000.0);
    }
}

pub fn primitive_mesh(kind: PrimitiveKind) -> Mesh {
    match kind {
        PrimitiveKind::Cube => cube(),
        PrimitiveKind::Sphere => sphere(16, 8),
        PrimitiveKind::Cylinder => cylinder(18),
        PrimitiveKind::Cone => cone(18),
        PrimitiveKind::Torus => torus(18, 8),
        PrimitiveKind::Wedge => wedge(),
        PrimitiveKind::Plane => plane(),
        PrimitiveKind::Imported => Mesh {
            vertices: Vec::new(),
            triangles: Vec::new(),
        },
    }
}

fn cube() -> Mesh {
    let vertices = vec![
        Vec3::new(-1.0, -1.0, -1.0),
        Vec3::new(1.0, -1.0, -1.0),
        Vec3::new(1.0, 1.0, -1.0),
        Vec3::new(-1.0, 1.0, -1.0),
        Vec3::new(-1.0, -1.0, 1.0),
        Vec3::new(1.0, -1.0, 1.0),
        Vec3::new(1.0, 1.0, 1.0),
        Vec3::new(-1.0, 1.0, 1.0),
    ];
    let triangles = vec![
        [0, 2, 1],
        [0, 3, 2],
        [4, 5, 6],
        [4, 6, 7],
        [0, 1, 5],
        [0, 5, 4],
        [2, 3, 7],
        [2, 7, 6],
        [1, 2, 6],
        [1, 6, 5],
        [3, 0, 4],
        [3, 4, 7],
    ];
    Mesh {
        vertices,
        triangles,
    }
}

fn plane() -> Mesh {
    Mesh {
        vertices: vec![
            Vec3::new(-5.0, 0.0, -5.0),
            Vec3::new(5.0, 0.0, -5.0),
            Vec3::new(5.0, 0.0, 5.0),
            Vec3::new(-5.0, 0.0, 5.0),
        ],
        triangles: vec![[0, 2, 1], [0, 3, 2]],
    }
}

fn sphere(segments: usize, rings: usize) -> Mesh {
    let mut vertices = Vec::new();
    for ring in 0..=rings {
        let phi = std::f32::consts::PI * ring as f32 / rings as f32;
        for segment in 0..segments {
            let theta = std::f32::consts::TAU * segment as f32 / segments as f32;
            vertices.push(Vec3::new(
                phi.sin() * theta.cos(),
                phi.cos(),
                phi.sin() * theta.sin(),
            ));
        }
    }
    let mut triangles = Vec::new();
    for ring in 0..rings {
        for segment in 0..segments {
            let next = (segment + 1) % segments;
            let a = ring * segments + segment;
            let b = ring * segments + next;
            let c = (ring + 1) * segments + segment;
            let d = (ring + 1) * segments + next;
            if ring > 0 {
                triangles.push([a, c, b]);
            }
            if ring + 1 < rings {
                triangles.push([b, c, d]);
            }
        }
    }
    Mesh {
        vertices,
        triangles,
    }
}

fn cylinder(segments: usize) -> Mesh {
    let mut vertices = vec![Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.0, 1.0, 0.0)];
    for y in [-1.0, 1.0] {
        for i in 0..segments {
            let angle = std::f32::consts::TAU * i as f32 / segments as f32;
            vertices.push(Vec3::new(angle.cos(), y, angle.sin()));
        }
    }
    let mut triangles = Vec::new();
    for i in 0..segments {
        let next = (i + 1) % segments;
        let bottom = 2 + i;
        let bottom_next = 2 + next;
        let top = 2 + segments + i;
        let top_next = 2 + segments + next;
        triangles.extend([
            [0, bottom_next, bottom],
            [1, top, top_next],
            [bottom, bottom_next, top_next],
            [bottom, top_next, top],
        ]);
    }
    Mesh {
        vertices,
        triangles,
    }
}

fn cone(segments: usize) -> Mesh {
    let mut vertices = vec![Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.0, 1.0, 0.0)];
    for i in 0..segments {
        let angle = std::f32::consts::TAU * i as f32 / segments as f32;
        vertices.push(Vec3::new(angle.cos(), -1.0, angle.sin()));
    }
    let mut triangles = Vec::new();
    for i in 0..segments {
        let next = (i + 1) % segments;
        triangles.push([0, 2 + next, 2 + i]);
        triangles.push([1, 2 + i, 2 + next]);
    }
    Mesh {
        vertices,
        triangles,
    }
}

fn torus(major_segments: usize, minor_segments: usize) -> Mesh {
    let mut vertices = Vec::new();
    for major in 0..major_segments {
        let u = std::f32::consts::TAU * major as f32 / major_segments as f32;
        for minor in 0..minor_segments {
            let v = std::f32::consts::TAU * minor as f32 / minor_segments as f32;
            let radius = 1.25 + 0.38 * v.cos();
            vertices.push(Vec3::new(
                radius * u.cos(),
                0.38 * v.sin(),
                radius * u.sin(),
            ));
        }
    }
    let mut triangles = Vec::new();
    for major in 0..major_segments {
        for minor in 0..minor_segments {
            let next_major = (major + 1) % major_segments;
            let next_minor = (minor + 1) % minor_segments;
            let a = major * minor_segments + minor;
            let b = next_major * minor_segments + minor;
            let c = next_major * minor_segments + next_minor;
            let d = major * minor_segments + next_minor;
            triangles.extend([[a, b, c], [a, c, d]]);
        }
    }
    Mesh {
        vertices,
        triangles,
    }
}

fn wedge() -> Mesh {
    Mesh {
        vertices: vec![
            Vec3::new(-1.0, -1.0, -1.0),
            Vec3::new(1.0, -1.0, -1.0),
            Vec3::new(-1.0, -1.0, 1.0),
            Vec3::new(1.0, -1.0, 1.0),
            Vec3::new(-1.0, 1.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
        ],
        triangles: vec![
            [0, 3, 1],
            [0, 2, 3],
            [2, 5, 3],
            [2, 4, 5],
            [0, 1, 5],
            [0, 5, 4],
            [0, 4, 2],
            [1, 3, 5],
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitives_have_valid_triangle_indices() {
        for kind in [
            PrimitiveKind::Cube,
            PrimitiveKind::Sphere,
            PrimitiveKind::Cylinder,
            PrimitiveKind::Cone,
            PrimitiveKind::Torus,
            PrimitiveKind::Wedge,
            PrimitiveKind::Plane,
        ] {
            let mesh = primitive_mesh(kind);
            assert!(!mesh.vertices.is_empty());
            assert!(!mesh.triangles.is_empty());
            assert!(
                mesh.triangles
                    .iter()
                    .flatten()
                    .all(|&i| i < mesh.vertices.len())
            );
        }
    }

    #[test]
    fn selection_wraps_in_both_directions() {
        let mut scene = Scene::default();
        scene.add_primitive(PrimitiveKind::Plane);
        scene.selected = 0;
        scene.select_next(-1);
        assert_eq!(scene.selected, 1);
        scene.select_next(1);
        assert_eq!(scene.selected, 0);
    }

    #[test]
    fn construction_plane_is_ten_units_square() {
        let mesh = primitive_mesh(PrimitiveKind::Plane);
        let width = mesh
            .vertices
            .iter()
            .map(|v| v.x)
            .fold(f32::NEG_INFINITY, f32::max)
            - mesh
                .vertices
                .iter()
                .map(|v| v.x)
                .fold(f32::INFINITY, f32::min);
        assert_eq!(width, 10.0);
    }

    #[test]
    fn duplicate_gets_a_unique_id_and_offset() {
        let mut scene = Scene::default();
        let original_id = scene.objects[0].id;
        assert!(scene.duplicate_selected());
        assert_ne!(scene.objects[1].id, original_id);
        assert_eq!(scene.objects[1].transform.position.x, 0.5);
    }
}
