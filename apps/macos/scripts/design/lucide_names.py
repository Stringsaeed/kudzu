import re, sys, pathlib
root = pathlib.Path(sys.argv[1])
names = set()
for d in ["components", "app"]:
    for p in (root / d).rglob("*.tsx"):
        s = p.read_text()
        for m in re.finditer(r'import\s*(type\s*)?\{([^}]*)\}\s*from\s*"lucide-react"', s, re.S):
            if m.group(1): continue
            for part in m.group(2).split(","):
                part = part.strip()
                if not part or part.startswith("type "): continue
                n = part.split(" as ")[0].strip()
                names.add(n)
    for p in (root / d).rglob("*.ts"):
        s = p.read_text()
        for m in re.finditer(r'import\s*(type\s*)?\{([^}]*)\}\s*from\s*"lucide-react"', s, re.S):
            if m.group(1): continue
            for part in m.group(2).split(","):
                part = part.strip()
                if not part or part.startswith("type "): continue
                names.add(part.split(" as ")[0].strip())
print("\n".join(sorted(names)))
