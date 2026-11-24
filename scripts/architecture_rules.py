"""Pure source-boundary rules shared by the gate and its regression fixtures."""
import re


def production_source(source):
    """Mask literals/comments and test items, preserving later production code."""
    tokens = re.compile(
        r'(?s)r(?P<hashes>\#{0,16})".*?"(?P=hashes)|"(?:\\.|[^"\\])*"|/\*.*?\*/|//[^\n]*|\'(?:\\.|[^\'\\])\''
    )
    production = tokens.sub(lambda match: re.sub(r'[^\n]', ' ', match[0]),
                            source)
    if re.match(r'\s*#!\[cfg\(test\)\]', production):
        return ''
    pattern = re.compile(r'#\[cfg\(test\)\]')
    while match := pattern.search(production):
        start = match.start()
        cursor = match.end()
        # Attributes after cfg(test) may contain braces; skip them first.
        while True:
            attr = re.match(r'\s*#\[[^]]*\]', production[cursor:])
            if attr is None:
                break
            cursor += attr.end()
        opening = re.search(r'[;{]', production[cursor:])
        if opening is None:
            production = production[:start]
            break
        cursor += opening.start()
        if production[cursor] == ';':
            end = cursor + 1
        else:
            depth, end = 1, cursor + 1
            while end < len(production) and depth:
                if production[end] == '{':
                    depth += 1
                elif production[end] == '}':
                    depth -= 1
                end += 1
        production = production[:start] + production[end:]
    return production


def _check_imports(source, name, forbidden):
    production = production_source(source)
    assert not re.search(forbidden + r'\s*::', production), name
    for imported in re.findall(r'\buse\s+([^;]+);', production):
        assert not re.search(forbidden, imported), name


def check_core_imports(source, name):
    _check_imports(source, name,
                   r'\b(features|extensions|application|persistence)\b')


def check_feature_imports(source, name):
    _check_imports(source, name, r'\b(application|persistence)\b')


def check_persistence_imports(source, name):
    _check_imports(source, name, r'\b(application|Application)\b')


def check_lock_usage(source, name):
    if str(name).replace('\\', '/').endswith('core/sync.rs'):
        return
    production = production_source(source)
    match = re.search(r'\.(lock|read|write)\(\)\s*\.(unwrap|expect)\b',
                      production)
    assert match is None, (name, match.group(0))
