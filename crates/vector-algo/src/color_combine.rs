use serde::{Deserialize, Serialize};
use vector_core::{PathCommand, VectorCoreError, VectorDocument, VectorPath};

/// Detailed statistics report for the Color Grouping operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ColorCombineReport {
    pub original_path_count: usize,
    pub number_of_color_groups: usize,
    pub combined_path_count: usize,
    pub group_details: Vec<ColorGroupDetail>,
}

/// Statistics for an individual color group.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ColorGroupDetail {
    pub fill_color: String,
    pub input_paths: usize,
    pub output_paths: usize,
}

pub struct ColorCombiner;

impl ColorCombiner {
    /// Groups SVG document paths by fill color and combines all sub-paths of each color into
    /// exactly ONE compound `VectorPath` per color group.
    ///
    /// Preserves natural Z-index layer ordering from the original SVG document sequence (`group_order` by first appearance)
    /// so background fills render at the bottom and foreground line art/details render safely on top.
    pub fn combine_document(
        doc: &VectorDocument,
    ) -> Result<(VectorDocument, ColorCombineReport), VectorCoreError> {
        let original_path_count = doc.paths.len();

        if original_path_count == 0 {
            return Ok((
                doc.clone(),
                ColorCombineReport {
                    original_path_count: 0,
                    number_of_color_groups: 0,
                    combined_path_count: 0,
                    group_details: Vec::new(),
                },
            ));
        }

        let mut combined_doc = VectorDocument::new();
        combined_doc.width = doc.width;
        combined_doc.height = doc.height;
        combined_doc.view_box = doc.view_box;

        let mut current_compound: Option<VectorPath> = None;
        let mut group_details = Vec::new();
        
        let mut current_group_id: Option<usize> = None;
        let mut paths_in_current_group = 0;
        let mut num_groups = 0;

        for path in &doc.paths {
            let color_key = path.fill.as_deref().unwrap_or("none").trim().to_string();

            if let Some(ref mut compound) = current_compound {
                if current_group_id == Some(path.group_id) {
                    // Combine into current compound path (Pure subpath concatenation, strictly 1:1)
                    for cmd in &path.commands {
                        compound.add_command(cmd.clone());
                    }
                    paths_in_current_group += 1;
                    continue;
                } else {
                    // Finalize previous compound path
                    combined_doc.paths.push(compound.clone());
                    group_details.push(ColorGroupDetail {
                        fill_color: compound.fill.clone().unwrap_or_else(|| "none".to_string()),
                        input_paths: paths_in_current_group,
                        output_paths: 1,
                    });
                }
            }

            // Start a new compound path
            current_group_id = Some(path.group_id);
            paths_in_current_group = 1;
            num_groups += 1;
            
            let mut new_path = VectorPath::new(Some(format!(
                "group_{}",
                num_groups
            )));
            new_path.fill = path.fill.clone();
            new_path.stroke = path.stroke.clone();
            new_path.stroke_width = path.stroke_width;
            new_path.fill_rule = Some("evenodd".to_string());
            new_path.group_id = path.group_id;

            for cmd in &path.commands {
                new_path.add_command(cmd.clone());
            }

            current_compound = Some(new_path);
        }

        // Finalize the last compound path
        if let Some(compound) = current_compound {
            combined_doc.paths.push(compound.clone());
            group_details.push(ColorGroupDetail {
                fill_color: compound.fill.clone().unwrap_or_else(|| "none".to_string()),
                input_paths: paths_in_current_group,
                output_paths: 1,
            });
        }

        let combined_path_count = combined_doc.paths.len();
        let number_of_color_groups = num_groups;

        let report = ColorCombineReport {
            original_path_count,
            number_of_color_groups,
            combined_path_count,
            group_details,
        };

        Ok((combined_doc, report))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vector_core::{PathCommand, Point2D};

    #[test]
    fn test_compound_path_color_grouping() {
        let mut doc = VectorDocument::new();

        // Color Group 1: Red (#FF0000) - 2 Separate Squares
        let mut r1 = VectorPath::new(Some("r1".to_string()));
        r1.fill = Some("#FF0000".to_string());
        r1.group_id = 1;
        r1.add_command(PathCommand::MoveTo(Point2D::new(0.0, 0.0)));
        r1.add_command(PathCommand::LineTo(Point2D::new(50.0, 0.0)));
        r1.add_command(PathCommand::LineTo(Point2D::new(50.0, 50.0)));
        r1.add_command(PathCommand::LineTo(Point2D::new(0.0, 50.0)));
        r1.add_command(PathCommand::Close);

        let mut r2 = VectorPath::new(Some("r2".to_string()));
        r2.fill = Some("#FF0000".to_string());
        r2.group_id = 1;
        r2.add_command(PathCommand::MoveTo(Point2D::new(25.0, 0.0)));
        r2.add_command(PathCommand::LineTo(Point2D::new(75.0, 0.0)));
        r2.add_command(PathCommand::LineTo(Point2D::new(75.0, 50.0)));
        r2.add_command(PathCommand::LineTo(Point2D::new(25.0, 50.0)));
        r2.add_command(PathCommand::Close);

        // Color Group 2: Blue (#0000FF) - 1 Square
        let mut b1 = VectorPath::new(Some("b1".to_string()));
        b1.fill = Some("#0000FF".to_string());
        b1.group_id = 2;
        b1.add_command(PathCommand::MoveTo(Point2D::new(100.0, 0.0)));
        b1.add_command(PathCommand::LineTo(Point2D::new(150.0, 0.0)));
        b1.add_command(PathCommand::LineTo(Point2D::new(150.0, 50.0)));
        b1.add_command(PathCommand::LineTo(Point2D::new(100.0, 50.0)));
        b1.add_command(PathCommand::Close);

        doc.paths.push(r1);
        doc.paths.push(r2);
        doc.paths.push(b1);

        let (combined_doc, report) = ColorCombiner::combine_document(&doc).unwrap();

        assert_eq!(report.original_path_count, 3);
        assert_eq!(report.number_of_color_groups, 2);
        assert_eq!(report.combined_path_count, 2);
        assert_eq!(combined_doc.paths.len(), 2);
    }
}
