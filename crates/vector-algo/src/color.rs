use vector_core::{VectorCoreError, VectorDocument};

pub struct ColorVariantEngine;

impl ColorVariantEngine {
    /// Applies a configurable color palette (or preset name) to a `VectorDocument`.
    pub fn apply_palette(
        doc: &VectorDocument,
        palette: &[String],
    ) -> Result<VectorDocument, VectorCoreError> {
        if palette.is_empty() {
            return Err(VectorCoreError::InvalidGeometry("Palette cannot be empty".to_string()));
        }

        let mut variant_doc = doc.clone();

        for (idx, path) in variant_doc.paths.iter_mut().enumerate() {
            let fill_color = &palette[idx % palette.len()];
            let stroke_color = &palette[(idx + 1) % palette.len()];

            path.fill = Some(fill_color.clone());
            if path.stroke.is_some() {
                path.stroke = Some(stroke_color.clone());
            }
        }

        Ok(variant_doc)
    }

    /// Resolves preset palette names ("pastel", "cyberpunk", "monochrome", "earth") to HEX color vectors.
    pub fn resolve_preset(preset: &str) -> Vec<String> {
        match preset.to_lowercase().as_str() {
            "pastel" => vec![
                "#FFB3BA".into(), "#FFDFBA".into(), "#FFFFBA".into(),
                "#BAFFC9".into(), "#BAE1FF".into()
            ],
            "cyberpunk" => vec![
                "#00F0FF".into(), "#FF007F".into(), "#7000FF".into(),
                "#FFE600".into(), "#00FF66".into()
            ],
            "monochrome" => vec![
                "#111111".into(), "#333333".into(), "#555555".into(),
                "#777777".into(), "#999999".into()
            ],
            "earth" => vec![
                "#2D5A27".into(), "#8B5A2B".into(), "#C2B280".into(),
                "#D2691E".into(), "#4A7023".into()
            ],
            _ => vec!["#000000".into()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vector_core::{PathCommand, Point2D, VectorPath};

    #[test]
    fn test_custom_color_palette_mapping() {
        let mut doc = VectorDocument::new();

        let mut p1 = VectorPath::new(Some("p1".into()));
        p1.add_command(PathCommand::MoveTo(Point2D::new(0.0, 0.0)));
        p1.add_command(PathCommand::LineTo(Point2D::new(10.0, 10.0)));

        let mut p2 = VectorPath::new(Some("p2".into()));
        p2.add_command(PathCommand::MoveTo(Point2D::new(10.0, 10.0)));
        p2.add_command(PathCommand::LineTo(Point2D::new(20.0, 20.0)));

        doc.paths.push(p1);
        doc.paths.push(p2);

        let custom_palette = vec!["#FF0000".to_string(), "#00FF00".to_string()];
        let variant = ColorVariantEngine::apply_palette(&doc, &custom_palette).unwrap();

        assert_eq!(variant.paths[0].fill.as_deref(), Some("#FF0000"));
        assert_eq!(variant.paths[1].fill.as_deref(), Some("#00FF00"));
    }

    #[test]
    fn test_preset_palette_resolution() {
        let cyberpunk = ColorVariantEngine::resolve_preset("cyberpunk");
        assert_eq!(cyberpunk.len(), 5);
        assert_eq!(cyberpunk[0], "#00F0FF");
    }
}
