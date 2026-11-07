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
    /// Opaque CAD vertices: position XYZ, colour RGB, alpha.
    pub opaque_vertices: Arc<Vec<[f32; 7]>>,
    /// Transparent CAD vertices: position XYZ, colour RGB, alpha.
    pub transparent_vertices: Arc<Vec<[f32; 7]>>,
    pub part_count: usize,
    pub occurrence_count: usize,
    pub triangle_count: usize,
    /// Summary of the detected source axes and field-fit transform.
    pub alignment: String,
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

    let mut opaque_vertices = Vec::with_capacity(model.triangle_count().saturating_mul(3));
    let mut transparent_vertices = Vec::new();
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
            &mut opaque_vertices,
            &mut transparent_vertices,
            0,
        )?;
    }

    if opaque_vertices.is_empty() && transparent_vertices.is_empty() {
        return Err("The STEP file was parsed, but no visible triangles were produced.".to_owned());
    }

    // Fit both material groups together so they share exactly the same transform.
    let mut all_vertices = Vec::with_capacity(opaque_vertices.len() + transparent_vertices.len());
    all_vertices.extend_from_slice(&opaque_vertices);
    all_vertices.extend_from_slice(&transparent_vertices);
    let alignment = fit_to_field(&mut all_vertices);
    let opaque_len = opaque_vertices.len();
    opaque_vertices.copy_from_slice(&all_vertices[..opaque_len]);
    transparent_vertices = all_vertices[opaque_len..].to_vec();

    let triangle_count = (opaque_vertices.len() + transparent_vertices.len()) / 3;
    Ok(StepScene {
        opaque_vertices: Arc::new(opaque_vertices),
        transparent_vertices: Arc::new(transparent_vertices),
        part_count: model.parts.len(),
        occurrence_count,
        triangle_count,
        alignment,
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
    opaque_output: &mut Vec<[f32; 7]>,
    transparent_output: &mut Vec<[f32; 7]>,
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
            let rgba = [colour[0], colour[1], colour[2], colour[3].clamp(0.0, 1.0)];
            let output = if rgba[3] < 0.999 { &mut *transparent_output } else { &mut *opaque_output };

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
                    rgba[0], rgba[1], rgba[2], rgba[3],
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
            opaque_output,
            transparent_output,
            depth + 1,
        )?;
    }

    active_parts.remove(&occurrence.part);
    Ok(())
}

fn fit_to_field(vertices: &mut [[f32; 7]]) -> String {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for vertex in vertices.iter() {
        for axis in 0..3 {
            min[axis] = min[axis].min(vertex[axis]);
            max[axis] = max[axis].max(vertex[axis]);
        }
    }

    let extent = [
        (max[0] - min[0]).max(0.001),
        (max[1] - min[1]).max(0.001),
        (max[2] - min[2]).max(0.001),
    ];
    let vertical_axis = (0..3)
        .min_by(|a, b| extent[*a].total_cmp(&extent[*b]))
        .unwrap_or(2);
    let horizontal_axes: Vec<usize> = (0..3)
        .filter(|axis| *axis != vertical_axis)
        .collect();
    let x_axis = horizontal_axes[0];
    let y_axis = horizontal_axes[1];

    // Use one uniform scale so structural proportions and vertical heights
    // remain faithful. Center the footprint in the 144-inch field and put its
    // lowest CAD point on the simulator floor (Z = 0).
    let scale = (144.0 / extent[x_axis]).min(144.0 / extent[y_axis]);
    let center_x = (min[x_axis] + max[x_axis]) * 0.5;
    let center_y = (min[y_axis] + max[y_axis]) * 0.5;
    let floor = min[vertical_axis];

    for vertex in vertices {
        let source = [vertex[0], vertex[1], vertex[2]];
        vertex[0] = (source[x_axis] - center_x) * scale + 72.0;
        vertex[1] = (source[y_axis] - center_y) * scale + 72.0;
        vertex[2] = (source[vertical_axis] - floor) * scale;
    }

    let aspect = extent[x_axis].max(extent[y_axis])
        / extent[x_axis].min(extent[y_axis]).max(0.001);
    let axis_name = ["X", "Y", "Z"];
    format!(
        "Auto-aligned {}-up CAD · source footprint {:.1} × {:.1} in · fit {:.4}× · aspect {:.3}",
        axis_name[vertical_axis],
        extent[x_axis],
        extent[y_axis],
        scale,
        aspect,
    )
}
