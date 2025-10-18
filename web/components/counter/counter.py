import json

# A simple in-memory store for the counter
counter_store = {"count": 0}

def load_template_context():
    """
    Called on GET requests. Returns the initial state of the component.
    """
    return {"count": json.dumps(counter_store["count"])}

def action_increment(**kwargs):
    """
    Called on POST requests with action="increment".
    Increments the counter and returns the new state.
    """
    counter_store["count"] += 1
    return {"count": json.dumps(counter_store["count"])}

def action_decrement(**kwargs):
    """
    Called on POST requests with action="decrement".
    Decrements the counter and returns the new state.
    """
    counter_store["count"] -= 1
    return {"count": json.dumps(counter_store["count"])}