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
from sqlalchemy.orm import DeclarativeBase

def orm_to_dict(obj):
    return {c.key: getattr(obj, c.key) for c in inspect(obj).mapper.column_attrs}

def call_user_function(user_func, *args, **kwargs):
    result = user_func(*args, **kwargs)

    if isinstance(result, list):
        # Check the first element to see if it's a model instance
        if result and hasattr(result[0], '__table__'):
            return [orm_to_dict(item) for item in result]
        else:
            return result # It's a list of something else, return as is
    elif hasattr(result, '__table__'):
        return orm_to_dict(result)
    elif isinstance(result, dict):
        return result
    
    # If it's a simple type, it will be handled by pythonize, otherwise it might fail
    return result

"#;