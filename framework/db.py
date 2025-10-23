from sqlalchemy import create_engine
from sqlalchemy.orm import sessionmaker

def initialize_database(db_url):
    """
    Initializes the database connection and returns a session factory.
    """
    if not db_url:
        return None
    print(f"Connecting to database at {db_url}")
    engine = create_engine(db_url)
    Session = sessionmaker(bind=engine)
    return Session()