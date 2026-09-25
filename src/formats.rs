use crate::{
    math::Vec3,
    scene::{Mesh, Scene},
};
use std::{
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug)]
pub struct FormatError(pub String);

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for FormatError {}

pub type Result<T> = std::result::Result<T, FormatError>;

pub fn save(scene: &Scene, path: &Path) -> Result<()> {
    match extension(path).as_str() {
        "forge" | "json" => save_native(scene, path),
        "obj" => export_obj(scene, path),
        "stl" => export_stl(scene, path),
        "step" | "stp" => export_step_via_freecad(scene, path),
        "blend" => export_blend_via_blender(scene, path),
        "f3d" => Err(FormatError(
            "Fusion 360's .f3d is proprietary and has no supported headless writer. Export .step, then import it in Fusion 360.".into(),
        )),
        ext => Err(FormatError(format!("unknown output format '.{ext}'; use .forge, .obj, .stl, .step, or .blend"))),
    }
}

pub fn load(path: &Path) -> Result<Scene> {
    match extension(path).as_str() {
        "forge" | "json" => load_native(path),
        "obj" => import_obj(path),
        "step" | "stp" => import_via_freecad(path),
        "blend" => import_via_blender(path),
        "f3d" => Err(FormatError(
            "Fusion 360's .f3d requires Fusion 360. Export STEP from Fusion, then open the .step file here.".into(),
        )),
        ext => Err(FormatError(format!("unknown input format '.{ext}'"))),
    }
}

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn save_native(scene: &Scene, path: &Path) -> Result<()> {
    let data = serde_json::to_string_pretty(scene).map_err(|e| FormatError(e.to_string()))?;
    fs::write(path, data)
        .map_err(|e| FormatError(format!("could not save {}: {e}", path.display())))
}

fn load_native(path: &Path) -> Result<Scene> {
    let data = fs::read_to_string(path)
        .map_err(|e| FormatError(format!("could not open {}: {e}", path.display())))?;
    let mut scene: Scene = serde_json::from_str(&data)
        .map_err(|e| FormatError(format!("invalid ForgeTUI project: {e}")))?;
    scene.repair_after_load();
    Ok(scene)
}

pub fn export_obj(scene: &Scene, path: &Path) -> Result<()> {
    let mut output = String::from("# ForgeTUI Wavefront OBJ export\n");
    let mut vertex_offset = 1;
    for object in scene.objects.iter().filter(|object| object.visible) {
        output.push_str(&format!("o {}\n", object.name.replace([' ', '#'], "_")));
        for vertex in &object.mesh.vertices {
            let world = object.transform.point(*vertex);
            output.push_str(&format!("v {:.7} {:.7} {:.7}\n", world.x, world.y, world.z));
        }
        for [a, b, c] in &object.mesh.triangles {
            output.push_str(&format!(
                "f {} {} {}\n",
                a + vertex_offset,
                b + vertex_offset,
                c + vertex_offset
            ));
        }
        vertex_offset += object.mesh.vertices.len();
    }
    fs::write(path, output)
        .map_err(|e| FormatError(format!("could not export {}: {e}", path.display())))
}

pub fn export_stl(scene: &Scene, path: &Path) -> Result<()> {
    let mut output = String::from("solid forgetui\n");
    for (_, _, triangle) in scene.world_triangles() {
        let normal = (triangle[1] - triangle[0])
            .cross(triangle[2] - triangle[0])
            .normalized();
        output.push_str(&format!(
            "  facet normal {} {} {}\n    outer loop\n",
            normal.x, normal.y, normal.z
        ));
        for vertex in triangle {
            output.push_str(&format!(
                "      vertex {} {} {}\n",
                vertex.x, vertex.y, vertex.z
            ));
        }
        output.push_str("    endloop\n  endfacet\n");
    }
    output.push_str("endsolid forgetui\n");
    fs::write(path, output)
        .map_err(|e| FormatError(format!("could not export {}: {e}", path.display())))
}

pub fn import_obj(path: &Path) -> Result<Scene> {
    let input = fs::read_to_string(path)
        .map_err(|e| FormatError(format!("could not open {}: {e}", path.display())))?;
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    for (line_number, raw) in input.lines().enumerate() {
        let line = raw.trim();
        if let Some(rest) = line.strip_prefix("v ") {
            let values: std::result::Result<Vec<f32>, _> =
                rest.split_whitespace().take(3).map(str::parse).collect();
            let values = values.map_err(|_| {
                FormatError(format!("invalid vertex on OBJ line {}", line_number + 1))
            })?;
            if values.len() != 3 {
                return Err(FormatError(format!(
                    "vertex on OBJ line {} needs three coordinates",
                    line_number + 1
                )));
            }
            vertices.push(Vec3::new(values[0], values[1], values[2]));
        } else if let Some(rest) = line.strip_prefix("f ") {
            let indices: Result<Vec<usize>> = rest
                .split_whitespace()
                .map(|token| {
                    let raw_index = token
                        .split('/')
                        .next()
                        .unwrap_or_default()
                        .parse::<isize>()
                        .map_err(|_| {
                            FormatError(format!("invalid face on OBJ line {}", line_number + 1))
                        })?;
                    if raw_index == 0 {
                        return Err(FormatError("OBJ indices start at 1".into()));
                    }
                    let resolved = if raw_index < 0 {
                        vertices.len() as isize + raw_index
                    } else {
                        raw_index - 1
                    };
                    if resolved < 0 || resolved as usize >= vertices.len() {
                        return Err(FormatError(format!(
                            "face index out of range on OBJ line {}",
                            line_number + 1
                        )));
                    }
                    Ok(resolved as usize)
                })
                .collect();
            let indices = indices?;
            for i in 1..indices.len().saturating_sub(1) {
                triangles.push([indices[0], indices[i], indices[i + 1]]);
            }
        }
    }
    if vertices.is_empty() || triangles.is_empty() {
        return Err(FormatError("OBJ contains no renderable triangles".into()));
    }
    let mut scene = Scene::default();
    scene.objects.clear();
    scene.name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Imported")
        .into();
    scene.add_mesh(
        scene.name.clone(),
        Mesh {
            vertices,
            triangles,
        },
    );
    Ok(scene)
}

fn command_exists(name: &str) -> bool {
    Command::new(name).arg("--version").output().is_ok()
}

fn temporary_obj(path: &Path) -> PathBuf {
    let mut tmp = std::env::temp_dir();
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("scene");
    tmp.push(format!("forgetui-{stem}-{}.obj", std::process::id()));
    tmp
}

fn export_blend_via_blender(scene: &Scene, path: &Path) -> Result<()> {
    if !command_exists("blender") {
        return Err(FormatError(
            ".blend export needs Blender on PATH. Install Blender, or export .obj/.stl now.".into(),
        ));
    }
    let temp = temporary_obj(path);
    export_obj(scene, &temp)?;
    let expression = format!(
        "import bpy; bpy.ops.wm.read_factory_settings(use_empty=True); bpy.ops.wm.obj_import(filepath={:?}); bpy.ops.wm.save_as_mainfile(filepath={:?})",
        temp.to_string_lossy(),
        path.to_string_lossy()
    );
    let result = Command::new("blender")
        .args(["--background", "--python-expr", &expression])
        .status();
    let _ = fs::remove_file(&temp);
    match result {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(FormatError(format!(
            "Blender conversion failed with {status}"
        ))),
        Err(error) => Err(FormatError(format!("could not start Blender: {error}"))),
    }
}

fn import_via_blender(path: &Path) -> Result<Scene> {
    if !command_exists("blender") {
        return Err(FormatError(".blend import needs Blender on PATH".into()));
    }
    let temp = temporary_obj(path);
    let expression = format!(
        "import bpy; bpy.ops.wm.open_mainfile(filepath={:?}); bpy.ops.wm.obj_export(filepath={:?}, export_materials=False)",
        path.to_string_lossy(),
        temp.to_string_lossy()
    );
    let status = Command::new("blender")
        .args(["--background", "--python-expr", &expression])
        .status()
        .map_err(|e| FormatError(format!("could not start Blender: {e}")))?;
    if !status.success() {
        return Err(FormatError(format!(
            "Blender conversion failed with {status}"
        )));
    }
    let scene = import_obj(&temp);
    let _ = fs::remove_file(&temp);
    scene
}

fn freecad_command() -> Option<&'static str> {
    ["FreeCADCmd", "freecadcmd"]
        .into_iter()
        .find(|name| command_exists(name))
}

fn export_step_via_freecad(scene: &Scene, path: &Path) -> Result<()> {
    let Some(freecad) = freecad_command() else {
        return Err(FormatError(
            "STEP export needs FreeCADCmd on PATH. Install FreeCAD, or export .stl/.obj now."
                .into(),
        ));
    };
    let temp = temporary_obj(path);
    export_obj(scene, &temp)?;
    let mut script = std::env::temp_dir();
    script.push(format!("forgetui-step-{}.py", std::process::id()));
    let source = format!(
        "import FreeCAD as App, Mesh, Part\ndoc=App.newDocument('ForgeTUI')\nmesh=Mesh.Mesh({:?})\nshape=Part.Shape()\nshape.makeShapeFromMesh(mesh.Topology, 0.05)\nobj=doc.addObject('Part::Feature', 'Model')\nobj.Shape=shape.removeSplitter()\ndoc.recompute()\nPart.export([obj], {:?})\n",
        temp.to_string_lossy(),
        path.to_string_lossy()
    );
    fs::write(&script, source).map_err(|e| FormatError(e.to_string()))?;
    let status = Command::new(freecad).arg(&script).status();
    let _ = fs::remove_file(&temp);
    let _ = fs::remove_file(&script);
    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(FormatError(format!(
            "FreeCAD STEP conversion failed with {status}"
        ))),
        Err(error) => Err(FormatError(format!("could not start FreeCAD: {error}"))),
    }
}

fn import_via_freecad(path: &Path) -> Result<Scene> {
    let Some(freecad) = freecad_command() else {
        return Err(FormatError("STEP import needs FreeCADCmd on PATH".into()));
    };
    let temp = temporary_obj(path);
    let mut script = std::env::temp_dir();
    script.push(format!("forgetui-import-{}.py", std::process::id()));
    let source = format!(
        "import Part, Mesh\nshape=Part.read({:?})\nvertices, facets=shape.tessellate(0.1)\nmesh=Mesh.Mesh((vertices, facets))\nmesh.write({:?})\n",
        path.to_string_lossy(),
        temp.to_string_lossy()
    );
    fs::write(&script, source).map_err(|e| FormatError(e.to_string()))?;
    let status = Command::new(freecad)
        .arg(&script)
        .status()
        .map_err(|e| FormatError(e.to_string()))?;
    let _ = fs::remove_file(&script);
    if !status.success() {
        return Err(FormatError(format!(
            "FreeCAD STEP conversion failed with {status}"
        )));
    }
    let scene = import_obj(&temp);
    let _ = fs::remove_file(&temp);
    scene
}

pub fn format_matrix() -> &'static str {
    "Fusion 360: .f3d read/write requires Fusion; use STEP interchange\nOnshape: cloud-native; use .step/.stp via FreeCADCmd\nBlender: .blend via Blender CLI\nAlways available: .forge, .obj, .stl"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::PrimitiveKind;

    #[test]
    fn native_round_trip_preserves_scene() {
        let path = std::env::temp_dir().join(format!("forgetui-test-{}.forge", std::process::id()));
        let mut original = Scene::default();
        original.add_primitive(PrimitiveKind::Sphere);
        save(&original, &path).unwrap();
        let loaded = load(&path).unwrap();
        fs::remove_file(path).unwrap();
        assert_eq!(loaded.objects.len(), 2);
        assert_eq!(loaded.objects[1].kind, PrimitiveKind::Sphere);
    }

    #[test]
    fn obj_round_trip_keeps_triangles() {
        let path = std::env::temp_dir().join(format!("forgetui-test-{}.obj", std::process::id()));
        export_obj(&Scene::default(), &path).unwrap();
        let loaded = import_obj(&path).unwrap();
        fs::remove_file(path).unwrap();
        assert_eq!(loaded.objects[0].mesh.triangles.len(), 12);
    }

    #[test]
    fn unsupported_extension_is_actionable() {
        let error = save(&Scene::default(), Path::new("thing.xyz")).unwrap_err();
        assert!(error.to_string().contains("unknown output format"));
    }
}
