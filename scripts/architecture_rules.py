"""Pure source-boundary rules shared by the gate and its regression fixtures."""
import re


def check_core_imports(source, name):
    production = source.split('#[cfg(test)]', 1)[0]
    production = re.sub(r'//[^\n]*|/\*.*?\*/', '', production, flags=re.S)
    forbidden = r'\b(features|extensions|application|persistence)\b'
    assert not re.search(forbidden + r'\s*::', production), name
    for imported in re.findall(r'\buse\s+([^;]+);', production):
        assert not re.search(forbidden, imported), name
