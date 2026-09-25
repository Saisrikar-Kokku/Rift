import re

with open(r'c:\Saisrikar@Kokku\Vibe Coding\Rift\Rift-rust\ui\screens\settings.html', 'r', encoding='utf-8') as f:
    text = f.read()

ids = re.findall(r'id=["\']([^"\']+)["\']', text)
print(f'Total IDs: {len(ids)}')
for id in sorted(set(ids)):
    if any(k in id.lower() for k in ['audio', 'sound', 'mic', 'format', 'enhance']):
        print(' -', id)
