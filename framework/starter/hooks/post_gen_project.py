import os
import binascii
import re

# Path to the generated config.yaml file
config_path = os.path.join(os.getcwd(), 'config.yaml')

# Generate a new secret key
secret_key = binascii.hexlify(os.urandom(32)).decode('utf-8')

# Read the content of the config.yaml file
with open(config_path, 'r', encoding='utf-8') as f:
    content = f.read()

# Replace the placeholder with the new secret key
content = re.sub(
    r'secret_key: "!!!REPLACE-ME-WITH-A-REAL-SECRET-KEY!!!"',
    f'secret_key: "{secret_key}"',
    content
)

# Write the updated content back to the config.yaml file
with open(config_path, 'w', encoding='utf-8') as f:
    f.write(content)