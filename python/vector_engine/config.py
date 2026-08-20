"""Layered configuration management using Pydantic Settings and PyYAML."""

import os
from pathlib import Path
from typing import Optional, Literal, Dict, Any, List
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


class SystemConfig(BaseModel):
    precision: PrecisionConfig = Field(default_factory=PrecisionConfig)
    execution: ExecutionConfig = Field(default_factory=ExecutionConfig)
    spatial_index: SpatialIndexConfig = Field(default_factory=SpatialIndexConfig)
    logging: LoggingConfig = Field(default_factory=LoggingConfig)


class EngineConfig(BaseModel):
    system: SystemConfig = Field(default_factory=SystemConfig)
    profiles: Dict[str, Any] = Field(default_factory=dict)

    @classmethod
    def load_from_yaml(cls, path: Path) -> "EngineConfig":
        """Load configuration from a YAML file."""
        if not path.exists():
            raise ConfigurationError(f"Config file not found: {path}")
        try:
            with open(path, "r", encoding="utf-8") as f:
                data = yaml.safe_load(f) or {}
            
            # Handle root-level legacy system configs transparently
            if "system" not in data and ("precision" in data or "execution" in data):
                system_keys = {"precision", "execution", "spatial_index", "logging"}
                sys_data = {k: v for k, v in data.items() if k in system_keys}
                prof_data = {k: v for k, v in data.items() if k not in system_keys}
                data = {"system": sys_data, "profiles": prof_data}
                
            return cls(**data)
        except Exception as e:
            raise ConfigurationError(f"Failed to parse config YAML: {e}") from e


def load_config(config_path: Optional[Path] = None) -> EngineConfig:
    """Load engine config with priority: argument > ENV (VECTOR_ENGINE_CONFIG) > config/config.yaml > default_config.yaml."""
    if config_path is not None:
        return EngineConfig.load_from_yaml(config_path)

    env_config_path = os.getenv("VECTOR_ENGINE_CONFIG")
    if env_config_path:
        return EngineConfig.load_from_yaml(Path(env_config_path))

    unified_path = Path(__file__).parents[2] / "config" / "config.yaml"
    if unified_path.exists():
        return EngineConfig.load_from_yaml(unified_path)

    default_path = Path(__file__).parents[2] / "config" / "default_config.yaml"
    if default_path.exists():
        return EngineConfig.load_from_yaml(default_path)

    return EngineConfig()
