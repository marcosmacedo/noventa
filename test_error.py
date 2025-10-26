def function_that_fails():
    raise ValueError("This is an intentional error")

def another_level():
    function_that_fails()

def call_user_function(func):
    try:
        func()
    except Exception as exc:
        # Rebuild a traceback that excludes the `call_user_function` frame
        import types

        tb = exc.__traceback__
        parts = []
        # Collect traceback frames but skip frames named call_user_function
        while tb is not None:
            if tb.tb_frame.f_code.co_name != "call_user_function":
                parts.append(tb)
            tb = tb.tb_next

        # Reconstruct a new traceback chain from the collected parts
        new_tb = None
        for part in reversed(parts):
            new_tb = types.TracebackType(new_tb, part.tb_frame, part.tb_lasti, part.tb_lineno)

        # If filtering removed all frames, fall back to the original traceback
        if new_tb is None:
            new_tb = exc.__traceback__

        # Re-raise the same exception but with the reconstructed traceback so
        # upstream (the interpreter) will print the shortened traceback.
        raise exc.with_traceback(new_tb)

if __name__ == "__main__":
    call_user_function(another_level)