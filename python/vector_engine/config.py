"""Layered configuration management using Pydantic Settings and PyYAML."""

import os
from pathlib import Path
from typing import Optional, Literal
import yaml
from pydantic import BaseModel, Field

from vector_engine.exceptions import ConfigurationError


class PrecisionConfig(BaseModel):
    epsilon: float = Field(default=1e-9, description="Epsilon threshold for float comparisons")
    numeric_backend: str = Field(default="f64", description="Numeric backend precision type")


class ExecutionConfig(BaseModel):
    parallelism: int = Field(default=0, description="Number of worker threads (0 = auto)")
    buffer_strategy: Literal["zero_copy", "copy"] = Field(default="zero_copy")


class SpatialIndexConfig(BaseModel):
    node_capacity: int = Field(default=16, description="Max node capacity for spatial index")
    index_type: str = Field(default="rtree")


class LoggingConfig(BaseModel):
    level: str = Field(default="INFO")
    format: str = Field(default="console")
    bridge_rust_tracing: bool = Field(default=True)


class EngineConfig(BaseModel):
    precision: PrecisionConfig = Field(default_factory=PrecisionConfig)
    execution: ExecutionConfig = Field(default_factory=ExecutionConfig)
    spatial_index: SpatialIndexConfig = Field(default_factory=SpatialIndexConfig)
    logging: LoggingConfig = Field(default_factory=LoggingConfig)

    @classmethod
    def load_from_yaml(cls, path: Path) -> "EngineConfig":
        """Load configuration from a YAML file."""
        if not path.exists():
            raise ConfigurationError(f"Config file not found: {path}")
        try:
            with open(path, "r", encoding="utf-8") as f:
                data = yaml.safe_load(f) or {}
            return cls(**data)
        except Exception as e:
            raise ConfigurationError(f"Failed to parse config YAML: {e}") from e


def load_config(config_path: Optional[Path] = None) -> EngineConfig:
    """Load engine config with priority: argument > ENV (VECTOR_ENGINE_CONFIG) > default_config.yaml."""
    if config_path is not None:
        return EngineConfig.load_from_yaml(config_path)

    env_config_path = os.getenv("VECTOR_ENGINE_CONFIG")
    if env_config_path:
        return EngineConfig.load_from_yaml(Path(env_config_path))

    default_path = Path(__file__).parents[2] / "config" / "default_config.yaml"
    if default_path.exists():
        return EngineConfig.load_from_yaml(default_path)

    return EngineConfig()
