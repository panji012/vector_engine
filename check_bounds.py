import xml.etree.ElementTree as ET

def get_paths(file):
    tree = ET.parse(file)
    root = tree.getroot()
    return root.findall('.//{http://www.w3.org/2000/svg}path')

print("Input Design_041 paths:")
for p in get_paths('input/Design_041.svg'):
    print(p.attrib.get('d')[:50])

print("\nOutput Design_041 paths:")
for p in get_paths('output/test_9/Design_041.svg'):
    print(p.attrib.get('d')[:50])
