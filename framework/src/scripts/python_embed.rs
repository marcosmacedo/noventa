pub const DB_PY: &str = r#"
from sqlalchemy import create_engine
from sqlalchemy.orm import sessionmaker, DeclarativeBase

class Base(DeclarativeBase):
    pass

def initialize_database(db_url):
    engine = create_engine(db_url)
    Base.metadata.bind = engine
    DBSession = sessionmaker(bind=engine)
    return DBSession()
"#;

pub const UTILS_PY: &str = r#"
from sqlalchemy.inspection import inspect

def orm_to_dict(obj, visited=None):
    if visited is None:
        visited = set()
    
    obj_id = id(obj)
    if obj_id in visited:
        return None
    
    visited.add(obj_id)

    d = {c.key: getattr(obj, c.key) for c in inspect(obj).mapper.column_attrs}

    for r in inspect(obj).mapper.relationships:
        related_obj = getattr(obj, r.key)
        if related_obj is not None:
            if r.uselist:
                d[r.key] = [orm_to_dict(o, visited) for o in related_obj]
            else:
                d[r.key] = orm_to_dict(related_obj, visited)
    
    visited.remove(obj_id)
    return d

from sqlalchemy.orm import object_mapper
from sqlalchemy.orm.exc import UnmappedInstanceError

def is_mapped_instance(obj):
    try:
        object_mapper(obj)
        return True
    except UnmappedInstanceError:
        return False

def deep_convert(data):
    if isinstance(data, dict):
        return {key: deep_convert(value) for key, value in data.items()}
    elif isinstance(data, list):
        return [deep_convert(item) for item in data]
    elif is_mapped_instance(data):
        return orm_to_dict(data)
    else:
        return data

import sys

def call_user_function(user_func, *args, **kwargs):
    try:
        result = user_func(*args, **kwargs)
        return deep_convert(result)
    except Exception as e:
        exc_type, exc_value, exc_tb = sys.exc_info()
        # Re-raise with original traceback preserved
        raise e.with_traceback(exc_tb)
"#;