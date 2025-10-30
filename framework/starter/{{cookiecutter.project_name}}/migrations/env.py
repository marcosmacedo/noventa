import os
import sys
import glob
import importlib.util
import yaml
from logging.config import fileConfig
from sqlalchemy import engine_from_config, pool, MetaData
from alembic import context

# Add the project root to the Python path
sys.path.append(os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))

config = context.config
if config.config_file_name is not None:
    fileConfig(config.config_file_name)

# --- Model Discovery ---
def import_models():
    project_root = os.path.abspath(os.path.join(os.path.dirname(__file__), '..'))
    # Search for models in components
    components_models_pattern = os.path.join(project_root, 'components', '**', '*_models.py')
    models_files = glob.glob(components_models_pattern, recursive=True)
    
    # Search for models in project root ./models
    root_models_pattern = os.path.join(project_root, 'models', '*.py')
    models_files.extend(glob.glob(root_models_pattern))
    
    bases = []
    for models_file in models_files:
        module_name = os.path.splitext(os.path.basename(models_file))[0]
        spec = importlib.util.spec_from_file_location(module_name, models_file)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        if hasattr(module, 'Base'):
            bases.append(module.Base)
    return bases

def get_aggregated_metadata():
    bases = import_models()
    aggregated_metadata = MetaData()
    for Base in bases:
        for table in Base.metadata.tables.values():
            table.to_metadata(aggregated_metadata)
    return aggregated_metadata

target_metadata = get_aggregated_metadata()
# --- End Model Discovery ---

def get_db_url_from_config():
    """Reads the database URL from the framework's config.yaml and makes it absolute."""
    project_root = os.path.abspath(os.path.join(os.path.dirname(__file__), '..'))
    config_path = os.path.join(project_root, 'config.yaml')
    
    with open(config_path, 'r') as f:
        framework_config = yaml.safe_load(f)
    
    db_url = framework_config.get('database')
    
    # If the URL is a relative SQLite path, make it absolute from the project root
    if db_url.startswith('sqlite:///'):
        db_file = db_url[len('sqlite:///'):]
        if not os.path.isabs(db_file):
            db_path = os.path.join(project_root, db_file)
            return f'sqlite:///{db_path}'
            
    return db_url

def run_migrations_online() -> None:
    """Run migrations in 'online' mode."""
    db_url = get_db_url_from_config()
    config.set_main_option('sqlalchemy.url', db_url)

    connectable = engine_from_config(
        config.get_section(config.config_ini_section, {}),
        prefix="sqlalchemy.",
        poolclass=pool.NullPool,
    )

    with connectable.connect() as connection:
        context.configure(
            connection=connection,
            target_metadata=target_metadata
        )

        with context.begin_transaction():
            context.run_migrations()

def run_migrations_offline() -> None:
    """Run migrations in 'offline' mode.

    This configures the context with just a URL
    and not an Engine, though an Engine is acceptable
    here as well.  By skipping the Engine creation
    we don't even need a DBAPI to be available.

    Calls to context.execute() here emit the given string to the
    script output.

    """
    url = get_db_url_from_config()
    context.configure(
        url=url,
        target_metadata=target_metadata,
        literal_binds=True,
        dialect_opts={"paramstyle": "named"},
    )

    with context.begin_transaction():
        context.run_migrations()


if context.is_offline_mode():
    run_migrations_offline()
else:
    run_migrations_online()
