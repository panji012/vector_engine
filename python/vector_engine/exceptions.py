"""Custom exception hierarchy for Vector Geometry Engine."""

class VectorEngineError(Exception):
    """Base exception class for all Vector Engine errors."""
    pass


class GeometryError(VectorEngineError):
    """Raised when a geometry primitive is invalid or malformed."""
    pass


class InvalidGeometryError(GeometryError):
    """Raised when coordinates or topology fail geometric validity rules."""
    pass


class TopologyError(VectorEngineError):
    """Raised when topological operations fail (e.g. self-intersections)."""
    pass


class SpatialIndexError(VectorEngineError):
    """Raised when spatial index queries or insertion fail."""
    pass


class IOError(VectorEngineError):
    """Raised when geometry parsing or serialization fails."""
    pass


class ConfigurationError(VectorEngineError):
    """Raised when configuration validation or file loading fails."""
    pass
