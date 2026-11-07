//! STEP CAD loading and conversion for the 3D field renderer.
//!
//! The STEP reader tessellates the source B-rep into indexed triangle meshes.
//! Assembly occurrences are expanded with their child-to-parent transforms,
//! then converted from the STEP model's length unit into field inches.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use oxideav_step::{StepModel, Transform};

#[derive(Debug)]
pub struct StepScene {
    /// Interleaved OpenGL vertices: position XYZ followed by colour RGB.
    pub vertices: Arc<Vec<[f32; 6]>>,
    pub part_count: usize,
    pub occurrence_count: usize,
    pub triangle_count: usize,
    pub warnings: Vec<String>,
}

pub fn load(path: &Path) -> Result<StepScene, String> {
    let bytes = std::fs::read(path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    let model = oxideav_step::read_step(&bytes)
        .map_err(|error| format!("STEP import failed: {error}"))?;

    let inches_per_model_unit = model.length_unit_metres * 39.370_078_740_157_48;
    if !inches_per_model_unit.is_finite() || inches_per_model_unit <= 0.0 {
        return Err("The STEP file declares an invalid length unit.".to_owned());
    }

    let mut vertices = Vec::with_capacity(model.triangle_count().saturating_mul(3));
    let mut active_parts = HashSet::new();
    let mut occurrence_count = 0usize;

    for root in &model.roots {
        append_occurrence(
            &model,
            root,
            &Transform::IDENTITY,
            inches_per_model_unit,
            &mut active_parts,
            &mut occurrence_count,
            &mut vertices,
            0,
        )?;
    }

    if vertices.is_empty() {
        return Err("The STEP file was parsed, but no visible triangles were produced.".to_owned());
    }

    // CAD assemblies are often authored around the world origin. Recenter the
    // imported field and fit its horizontal bounds to the simulator's 144 in
    // coordinate system so the robot, hives, and game pieces share one frame.
    fit_to_field(&mut vertices);

    let triangle_count = vertices.len() / 3;
    Ok(StepScene {
        vertices: Arc::new(vertices),
        part_count: model.parts.len(),
        occurrence_count,
        triangle_count,
        warnings: model.warnings,
    })
}

#[allow(clippy::too_many_arguments)]
fn append_occurrence(
    model: &StepModel,
    occurrence: &oxideav_step::Occurrence,
    parent_transform: &Transform,
    inches_per_model_unit: f64,
    active_parts: &mut HashSet<usize>,
    occurrence_count: &mut usize,
    output: &mut Vec<[f32; 6]>,
    depth: usize,
) -> Result<(), String> {
    const MAX_ASSEMBLY_DEPTH: usize = 256;
    if depth > MAX_ASSEMBLY_DEPTH {
        return Err("The STEP assembly exceeds the supported nesting depth.".to_owned());
    }

    let part = model.parts.get(occurrence.part)
        .ok_or_else(|| format!("STEP assembly references missing part index {}.", occurrence.part))?;

    // A cycle in the assembly graph is malformed. Skip that branch instead
    // of recursing forever; repeated instances on separate branches remain valid.
    if !active_parts.insert(occurrence.part) {
        return Ok(());
    }

    *occurrence_count += 1;
    let world_transform = parent_transform.compose(&occurrence.transform);

    for shape in &part.shapes {
        for (triangle_index, triangle) in shape.mesh.triangles.iter().enumerate() {
            let colour = shape.triangle_colours.get(triangle_index)
                .and_then(|colour| *colour)
                .or(shape.colour)
                .unwrap_or([0.62, 0.64, 0.62, 1.0]);
            let rgb = [colour[0], colour[1], colour[2]];

            for &vertex_index in triangle {
                let Some(position) = shape.mesh.positions.get(vertex_index as usize) else {
                    active_parts.remove(&occurrence.part);
                    return Err(format!("STEP shape #{} contains an invalid triangle index.", shape.item));
                };
                let p = world_transform.apply(*position);
                output.push([
                    (p[0] * inches_per_model_unit) as f32,
                    (p[1] * inches_per_model_unit) as f32,
                    (p[2] * inches_per_model_unit) as f32,
                    rgb[0], rgb[1], rgb[2],
                ]);
            }
        }
    }

    for child in &part.children {
        append_occurrence(
            model,
            child,
            &world_transform,
            inches_per_model_unit,
            active_parts,
            occurrence_count,
            output,
            depth + 1,
        )?;
    }

    active_parts.remove(&occurrence.part);
    Ok(())
}

fn fit_to_field(vertices: &mut [[f32; 6]]) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for vertex in vertices.iter() {
        for axis in 0..3 {
            min[axis] = min[axis].min(vertex[axis]);
            max[axis] = max[axis].max(vertex[axis]);
        }
    }

    let width = (max[0] - min[0]).max(0.001);
    let depth = (max[1] - min[1]).max(0.001);
    let scale = (144.0 / width).min(144.0 / depth);
    let offset_x = 72.0 - (min[0] + max[0]) * 0.5 * scale;
    let offset_y = 72.0 - (min[1] + max[1]) * 0.5 * scale;
    let offset_z = -min[2] * scale;

    for vertex in vertices {
        vertex[0] = vertex[0] * scale + offset_x;
        vertex[1] = vertex[1] * scale + offset_y;
        vertex[2] = vertex[2] * scale + offset_z;
    }
}
