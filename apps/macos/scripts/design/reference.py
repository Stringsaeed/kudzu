# Reference OKLCH -> sRGB for every Kudzu token, parsed straight from globals.css.
# Uses the CSS Color 4 sample-code path (OKLab -> LMS -> XYZ D65 -> linear sRGB), independent of
# the Swift port's direct Ottosson matrices.
import re, sys, math
css = open(sys.argv[1]).read()

def block(selector):
    m = re.search(r'(?m)^' + re.escape(selector) + r'\s*\{(.*?)^\}', css, re.S)
    return dict(re.findall(r'--([\w-]+):\s*([^;]+);', m.group(1)))

root, dark = block(':root'), block('.dark')
vib, vibdark = block('.vibrant'), block('.vibrant.dark')

def resolve(table, name, fallback=None):
    v = table.get(name) if name in table else (fallback or {}).get(name)
    v = v.strip()
    m = re.fullmatch(r'var\(--([\w-]+)\)', v)
    if m: return resolve(table, m.group(1), fallback)
    return v

def parse(v):
    m = re.fullmatch(r'oklch\(([\d.]+)\s+([\d.]+)\s+([\d.]+)(?:\s*/\s*([\d.]+)(%?))?\)', v)
    l, c, h = float(m.group(1)), float(m.group(2)), float(m.group(3))
    a = 1.0
    if m.group(4): a = float(m.group(4)) / (100 if m.group(5) else 1)
    return l, c, h, a

def mul(M, v): return [sum(M[i][j]*v[j] for j in range(3)) for i in range(3)]
OKLab_to_LMS = [[1.0, 0.3963377773761749, 0.2158037573099136],[1.0, -0.1055613458156586, -0.0638541728258133],[1.0, -0.0894841775298119, -1.2914855480194092]]
LMS_to_XYZ = [[1.2268798758459243, -0.5578149944602171, 0.2813910456659647],[-0.0405757452148008, 1.1122868032803170, -0.0717110580655164],[-0.0763729366746601, -0.4214933324022432, 1.5869240198367816]]
XYZ_to_lin = [[12831/3959, -329/214, -1974/3959],[-851781/878810, 1648619/878810, 36519/878810],[705/12673, -2585/12673, 705/667]]

def to_srgb(l, c, h, a):
    rad = h * math.pi / 180
    lab = [l, c*math.cos(rad), c*math.sin(rad)]
    lms = [x**3 for x in mul(OKLab_to_LMS, lab)]
    lin = mul(XYZ_to_lin, mul(LMS_to_XYZ, lms))
    def g(x):
        s = -1 if x < 0 else 1; x = abs(x)
        y = 1.055 * x**(1/2.4) - 0.055 if x > 0.0031308 else 12.92 * x
        return min(1.0, max(0.0, s*y))
    return [g(x) for x in lin] + [a]

TOKENS = ["background","foreground","card","card-foreground","popover","popover-foreground","primary","primary-foreground","secondary","secondary-foreground","muted","muted-foreground","accent","accent-foreground","destructive","border","input","ring","chart-1","chart-2","chart-3","chart-4","chart-5","sidebar","sidebar-foreground","sidebar-primary","sidebar-primary-foreground","sidebar-accent","sidebar-accent-foreground","sidebar-border","sidebar-ring","surface","surface-raised","hairline","faint","iris","iris-soft","iris-foreground","iris-contrast","chip-time","chip-clipboard","chip-cursor","chip-form","chip-error","chip-neutral","success","warning","favorite"]

def camel(n):
    p = n.split('-'); return p[0] + ''.join(x[:1].upper() + x[1:] for x in p[1:])
def fmt(rgba): return "(" + ", ".join("%.6f" % x for x in rgba) + ")"

rows = []
for t in TOKENS:
    lv = resolve(root, t); dv = resolve(dark, t, root)
    rows.append((camel(t), to_srgb(*parse(lv)), to_srgb(*parse(dv))))
rows.append(("sidebarVibrant", to_srgb(*parse(vib["sidebar"])), to_srgb(*parse(vibdark["sidebar"]))))
thumb = to_srgb(*parse("oklch(0.5 0 0 / 0.25)"))
rows.append(("scrollbarThumb", thumb, thumb))
print("    static let expected: [(name: String, light: RGBA, dark: RGBA)] = [")
for n, l, d in rows: print('        ("%s", %s, %s),' % (n, fmt(l), fmt(d)))
print("    ]")
