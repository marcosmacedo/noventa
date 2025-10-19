from .counter_models import Counter

def get_or_create_counter(db):
    counter = db.query(Counter).first()
    if not counter:
        counter = Counter(count=0)
        db.add(counter)
        db.commit()
    return counter

def load_template_context(request, db=None, **kwargs):
    """
    Called on GET requests. Returns the initial state of the component.
    """
    if not db:
        return {"error": "Database connection not available"}
    
    counter = get_or_create_counter(db)
    return {
        "count": counter.count,
        "items": ["apple", "banana", "cherry"],
        "user": {"name": "John Doe"},
        "kwargs": kwargs
    }

def action_increment(request, db=None, **kwargs):
    """
    Called on POST requests with action="increment".
    Increments the counter and returns the new state.
    """
    if not db:
        return {"error": "Database connection not available"}

    counter = get_or_create_counter(db)
    counter.count += 1
    db.commit()
    
    return {
        "count": counter.count,
        "items": ["apple", "banana", "cherry"],
        "user": {"name": "John Doe"}
    }

def action_decrement(request, db=None, **kwargs):
    """
    Called on POST requests with action="decrement".
    Decrements the counter and returns the new state.
    """
    if not db:
        return {"error": "Database connection not available"}

    counter = get_or_create_counter(db)
    counter.count -= 1
    db.commit()

    return {
        "count": counter.count,
        "items": ["apple", "banana", "cherry"],
        "user": {"name": "John Doe"}
    }

def action_upload(request, **kwargs):
    """
    Called on POST requests with action="upload".
    Handles file uploads.
    """
    file = request.files.get("file")
    if file:
        data = file.read()
        print(f"Uploaded file '{file.filename}' with size: {len(data)} bytes")
    return {
        "count": counter_store["count"],
        "items": ["apple", "banana", "cherry"],
        "user": {"name": "John Doe"}
    }