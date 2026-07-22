"""EPS export module producing Adobe PostScript 3.0 EPS files."""

import json
from pathlib import Path
from typing import Dict, Any, Union, Optional

from vector_engine.exceptions import IOError


def export_eps(
    polygon: Dict[str, Any],
    output_path: Optional[Union[str, Path]] = None,
    title: str = "Vector Engine Export"
) -> str:
    """Export a polygon object to Adobe PostScript 3.0 EPS format.

    If output_path is provided, writes the EPS file to disk and returns the content string.
    """
    try:
        from vector_engine import _native
        poly_json = json.dumps(polygon)
        eps_content = _native.export_eps_json(poly_json, title)

        if output_path is not None:
            path = Path(output_path)
            path.write_text(eps_content, encoding="utf-8")

        return eps_content
    except Exception as e:
        raise IOError(f"EPS Export failed: {e}") from e
