import json

def render():
    return {
        "greeting": json.dumps("Hello"),
        "name": json.dumps("World")
    }