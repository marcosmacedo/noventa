from sqlalchemy import Column, Integer
from sqlalchemy.ext.declarative import declarative_base

Base = declarative_base()

class Counter(Base):
    __tablename__ = 'counter'
    id = Column(Integer, primary_key=True)
    count = Column(Integer, default=0)