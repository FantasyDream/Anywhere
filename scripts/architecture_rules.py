"""Pure source-boundary rules shared by the gate and its regression fixtures."""
import re


def _check_imports(source, name, forbidden):
    production = source.split('#[cfg(test)]', 1)[0]
    production = re.sub(r'//[^\n]*|/\*.*?\*/', '', production, flags=re.S)
    assert not re.search(forbidden + r'\s*::', production), name
    for imported in re.findall(r'\buse\s+([^;]+);', production):
        assert not re.search(forbidden, imported), name


def check_core_imports(source, name):
    _check_imports(source, name, r'\b(features|extensions|application|persistence)\b')


def check_feature_imports(source, name):
    _check_imports(source, name, r'\b(application|persistence)\b')
