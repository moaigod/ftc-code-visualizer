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
    // Use a deliberately coarser tessellation for an interactive field view.
    // This reduces curved-surface triangle counts while retaining the overall
    // shape of field structures. AP242 files that already contain triangles
    // cannot be re-tessellated by this setting.
    let options = oxideav_step::ReadOptions {
        tolerance: oxideav_step::Tolerance {
            relative: 0.01,
            absolute: None,
            max_angle: 0.35,
        },
        ..Default::default()
    };
    let model = oxideav_step::read_step_with(&bytes, &options)
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

    // Fit both material groups using shared bounds, then collapse sub-quarter-inch
    // tessellation detail. This is a display mesh, not an engineering export:
    // 0.25 in vertex clustering keeps field-scale structure while removing
    // excessive AP242 tessellation density.
    let alignment = fit_material_groups(&mut opaque_vertices, &mut transparent_vertices);
    let opaque_before = opaque_vertices.len() / 3;
    let transparent_before = transparent_vertices.len() / 3;
    let opaque_removed = simplify_display_mesh(&mut opaque_vertices, 0.25, 0.0008);
    let transparent_removed = simplify_display_mesh(&mut transparent_vertices, 0.25, 0.0008);

    let triangle_count = (opaque_vertices.len() + transparent_vertices.len()) / 3;
    let transparent_min_alpha = transparent_vertices.iter()
        .map(|vertex| vertex[6])
        .fold(1.0_f32, f32::min);
    let transparent_max_alpha = transparent_vertices.iter()
        .map(|vertex| vertex[6])
        .fold(0.0_f32, f32::max);
    let mut warnings = model.warnings;
    if transparent_vertices.is_empty() {
        warnings.push("No alpha-transparent triangles were identified in the STEP styles.".to_owned());
    } else if transparent_min_alpha >= 0.999 {
        warnings.push("Transparent geometry has no usable alpha values.".to_owned());
    }
    warnings.push(format!(
        "Display mesh: {triangle_count} triangles (opaque {opaque_before}→{}, transparent {transparent_before}→{}) · removed {} tiny/degenerate faces · transparent alpha {:.2}–{:.2}",
        opaque_vertices.len() / 3,
        transparent_vertices.len() / 3,
        opaque_removed + transparent_removed,
        transparent_min_alpha,
        transparent_max_alpha,
    ));
    Ok(StepScene {
        opaque_vertices: Arc::new(opaque_vertices),
        transparent_vertices: Arc::new(transparent_vertices),
        part_count: model.parts.len(),
        occurrence_count,
        triangle_count,
        alignment,
        warnings,
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

/// Reduce tessellation for a field-scale display. Each triangle is snapped to
/// a regular 3D grid; triangles that collapse or become visually negligible
/// are discarded. Colours and alpha remain attached to their original face.
fn simplify_display_mesh(vertices: &mut Vec<[f32; 7]>, grid: f32, min_area: f32) -> usize {
    let mut simplified = Vec::with_capacity(vertices.len());
    let mut removed = 0usize;

    for triangle in vertices.chunks_exact(3) {
        let mut snapped = [[0.0_f32; 7]; 3];
        for corner in 0..3 {
            snapped[corner] = triangle[corner];
            for axis in 0..3 {
                snapped[corner][axis] = (triangle[corner][axis] / grid).round() * grid;
            }
        }

        let same = |a: &[f32; 7], b: &[f32; 7]| {
            a[0] == b[0] && a[1] == b[1] && a[2] == b[2]
        };
        if same(&snapped[0], &snapped[1]) || same(&snapped[1], &snapped[2]) || same(&snapped[2], &snapped[0]) {
            removed += 1;
            continue;
        }

        let a = glam::Vec3::new(snapped[0][0], snapped[0][1], snapped[0][2]);
        let b = glam::Vec3::new(snapped[1][0], snapped[1][1], snapped[1][2]);
        let d = glam::Vec3::new(snapped[2][0], snapped[2][1], snapped[2][2]);
        let area = (b - a).cross(d - a).length() * 0.5;
        if !area.is_finite() || area < min_area {
            removed += 1;
            continue;
        }

        simplified.extend_from_slice(&snapped);
    }

    vertices.clear();
    *vertices = simplified;
    removed
}

fn fit_material_groups(opaque: &mut [[f32; 7]], transparent: &mut [[f32; 7]]) -> String {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for vertex in opaque.iter().chain(transparent.iter()) {
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

    let scale = (144.0 / extent[x_axis]).min(144.0 / extent[y_axis]);
    let center_x = (min[x_axis] + max[x_axis]) * 0.5;
    let center_y = (min[y_axis] + max[y_axis]) * 0.5;
    let floor = min[vertical_axis];

    for vertex in opaque.iter_mut().chain(transparent.iter_mut()) {
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
        axis_name[vertical_axis], extent[x_axis], extent[y_axis], scale, aspect,
    )
}
