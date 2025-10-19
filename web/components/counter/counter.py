# A simple in-memory store for the counter
counter_store = {"count": 0}

def load_template_context(request):
    """
    Called on GET requests. Returns the initial state of the component.
    """
    print("GET request path:", request.path)
    return {
        "count": counter_store["count"],
        "items": ["apple", "banana", "cherry"]
    }

def action_increment(request):
    """
    Called on POST requests with action="increment".
    Increments the counter and returns the new state.
    """
    print("POST request path for increment:", request.path)
    print("Form data:", request.form)
    counter_store["count"] += 1
    return {
        "count": counter_store["count"],
        "items": ["apple", "banana", "cherry"]
    }

def action_decrement(request):
    """
    Called on POST requests with action="decrement".
    Decrements the counter and returns the new state.
    """
    print("POST request path for decrement:", request.path)
    counter_store["count"] -= 1
    return {
        "count": counter_store["count"],
        "items": ["apple", "banana", "cherry"]
    }

def action_upload(request):
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
        "items": ["apple", "banana", "cherry"]
    }