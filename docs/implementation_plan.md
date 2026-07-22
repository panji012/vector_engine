# Goal
Fix the phantom origin node issue and implement missing SVG DOM `transform` matrix baking so that shapes maintain their correct scale, position, and orientation across coordinate spaces.

## Open Questions
- Should we attempt to bake transforms into the `stroke-width` property? For non-uniform scaling (e.g. `scale(2, 1)`), stroke-width becomes complex. Standard behavior for flatteners is usually to apply a uniform scale approximation or just ignore it if the user only cares about filling compound paths. We will proceed by approximating the uniform scaling factor for `stroke-width`.

## Proposed Changes

### 1. `vector-io/src/svg.rs`
- **Cursor Tracking Fix**: Audit `parse_d_attribute` to ensure `current_pt` accurately evaluates the absolute sum of local relative commands BEFORE being subjected to a transform matrix.
- **Transform Parsing**: Introduce a `current_transform: svgtypes::Transform` parameter to `traverse_node`.
- **Inheritance**: At each `<g>` or element node, attempt to parse the `"transform"` string using `svgtypes::Transform::from_str`. Use `append()` to stack inherited transformations down the DOM tree.
- **Baking into Absolute Geometry**: When finalizing `MoveTo`, `LineTo`, `QuadTo`, or `CubicTo` variants in `parse_path_element`, `parse_rect_element`, `parse_circle_element`, etc., execute `current_transform.apply(x, y)` to permanently bake the transformed global coordinate into the final `VectorPath`.

## Verification Plan
- Use the existing tests in `color_combine.rs` and `reconstruct.rs` to ensure nothing breaks. 
- Create a test case where a path starts with a relative MoveTo inside a translated group, asserting that its resulting absolute points correctly reflect both the relative displacement and the inherited group translation.
