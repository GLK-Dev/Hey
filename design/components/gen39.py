# Доски недостающих компонентов: радио, селект, комбобокс, степпер, полоса прокрутки,
# разделитель, подсказка, хлебные крошки, сегменты, чипы, заголовок таблицы, дерево,
# спиннер, цвет, выделение текста, ряд кнопок диалога. Три темы.
import sys, os
sys.path.insert(0, '/home/claude/sys')
os.chdir('/home/claude/sys')
from gen35 import *  # THEMES, helpers, MONO, SANS, CH, bevel, button, field, check, toggle, list_rows, menu, cr

def chev(c, d='down', s=10):
    rot = {'down': 0, 'right': -90, 'up': 180}[d]
    return f'<svg width="{s}" height="{s}" viewBox="0 0 10 10" style="flex:none;transform:rotate({rot}deg)"><path d="M1.5 3.5L5 7l3.5-3.5" fill="none" stroke="{c}" stroke-width="1.6"/></svg>'

def box_css(T, w=None):
    st = T['style']
    base = f"display:flex;align-items:center;justify-content:space-between;height:36px;padding:0 12px;box-sizing:border-box;font-size:14px;color:{T['text']};background:{T['document']};"
    if w: base += f"width:{w}px;"
    if st == 'bevel': base += bevel(T, True)
    elif st == 'hud': base += f"box-shadow:inset 0 0 0 1px {T['line']};font-family:{MONO};font-size:13px;"
    else: base += f"border-bottom:2px solid {T['line']};"
    return base

def surf_raised(T):
    st = T['style']
    if st == 'bevel': return f"background:{T['panel']};{bevel(T)}"
    if st == 'hud': return f"background:{T['document']};box-shadow:0 0 0 1px {T['accentDark']}"
    return f"background:{T['control']};box-shadow:0 0 0 1px {T['line']},0 6px 18px rgba(0,0,0,.5);{CH(T['chamfer'][1])}"

# ---- 1. радиокнопки
def radio(T, on, label, disabled=False):
    st = T['style']; fg = T['textDisabled'] if disabled else T['text']
    if st == 'bevel':
        box = f"width:18px;height:18px;border-radius:50%;box-sizing:border-box;background:{T['document']};{bevel(T, True)};display:flex;align-items:center;justify-content:center"
        dot = f'<span style="width:8px;height:8px;border-radius:50%;background:{"#1A1A1A" if not disabled else T["textDisabled"]}"></span>' if on else ''
    else:
        col = T['textDisabled'] if disabled else (T['accent'] if on else T['line'])
        box = f"width:18px;height:18px;box-sizing:border-box;border:2px solid {col};background:{T['document']};transform:rotate(45deg);display:flex;align-items:center;justify-content:center"
        dot = f'<span style="width:8px;height:8px;background:{col};transform:rotate(-45deg)"></span>' if on else ''
    return f'<span style="display:inline-flex;align-items:center;gap:10px;font-size:14px;color:{fg}"><span style="{box}">{dot}</span>{label}</span>'

def radios(T):
    return ('<div style="display:flex;flex-direction:column;gap:12px">' +
            radio(T, True, 'Вариант А') + radio(T, False, 'Вариант Б') + radio(T, False, 'Отключено', True) + '</div>')

# ---- 2. выпадающий список (закрыт / открыт)
def select(T, text, opened=False, w=220):
    head = f'<span style="{box_css(T, w)}"><span>{text}</span>{chev(T["textMuted"])}</span>'
    if not opened: return head
    items = [('Обычный', 'Ctrl+1'), ('Крупный', 'Ctrl+2'), ('-', ''), ('Моноширинный', 'Ctrl+3')]
    return f'<div>{head}<div style="margin-top:4px">{menu(T, items, sel=1, w=w)}</div></div>'

# ---- 3. комбобокс: поле + подсказки
def combo(T, w=240):
    st = T['style']
    inp = f'<span style="{box_css(T, w)}"><span>Мар<span style="background:{T["selection"]};color:{T["onSelection"]}">кет</span></span>{chev(T["textMuted"])}</span>'
    rows = [('Маркет', 'приложения'), ('Маркет (сеть)', 'поиск'), ('Марки', 'коллекция')]
    return f'<div>{inp}<div style="margin-top:4px">{list_rows(T, rows, sel=0, w=str(w)+"px")}</div></div>'

# ---- 4. степпер числа
def stepper(T, val='12', w=120):
    st = T['style']; btn = f'width:22px;height:17px;display:flex;align-items:center;justify-content:center;background:{T["control"] if st!="hud" else "transparent"};color:{T["text"]};font-size:10px;line-height:1'
    if st == 'bevel': btn += ';' + bevel(T)
    return (f'<span style="{box_css(T, w)};padding:0 0 0 12px">'
            f'<span>{val}</span>'
            f'<span style="display:flex;flex-direction:column;gap:2px;height:32px;margin-right:2px">'
            f'<span style="{btn}">▲</span><span style="{btn}">▼</span></span></span>')

# ---- 5. полоса прокрутки: вертикальная и горизонтальная, три состояния
def scrollbar(T, state='normal', vertical=True, length=140):
    st = T['style']
    track = {'bevel': '#1E2630', 'hud': T['document'], 'flat': T['document']}[st]
    if st == 'bevel':
        thumb = {'normal': 'linear-gradient(#46515F,#333D49)', 'hover': 'linear-gradient(#526072,#3C4856)', 'drag': 'linear-gradient(#FFDD4D,#E6B800)'}[state]
        tb = bevel(T)
    elif st == 'hud':
        thumb = {'normal': T['line'], 'hover': T['textDisabled'], 'drag': T['accent']}[state]; tb = ''
    else:
        thumb = {'normal': T['control'], 'hover': T['controlHover'], 'drag': T['accent']}[state]; tb = ''
    th = 0.42 * length
    if vertical:
        tk = f'position:relative;width:12px;height:{length}px;background:{track};'
        tm = f'position:absolute;left:0;right:0;top:{length*0.18:.0f}px;height:{th:.0f}px;background:{thumb};{tb}'
    else:
        tk = f'position:relative;width:{length}px;height:12px;background:{track};'
        tm = f'position:absolute;top:0;bottom:0;left:{length*0.18:.0f}px;width:{th:.0f}px;background:{thumb};{tb}'
    return f'<span style="display:inline-block;{tk}"><span style="{tm}"></span></span>'

def scrollbars(T):
    cells = ''
    for s, n in [('normal', 'обычная'), ('hover', 'наведение'), ('drag', 'тянем')]:
        cells += f'<div style="display:flex;flex-direction:column;align-items:center;gap:8px">{scrollbar(T, s, True)}{lab(T, n)}</div>'
    row = f'<div style="display:flex;gap:26px;align-items:flex-start">{cells}</div>'
    hz = ''.join(f'<div style="margin-top:10px">{scrollbar(T, s, False, 220)}{lab(T, n)}</div>' for s, n in [('normal', 'обычная'), ('drag', 'тянем')])
    return row + hz

# ---- 6. разделитель панелей
def splitter(T, state='idle', vertical=True):
    st = T['style']
    handle = {'idle': T['line'], 'hover': T['accent'], 'drag': T['accent']}[state]
    if st == 'bevel': handle = {'idle': '#4A5563', 'hover': '#FFCC00', 'drag': '#FFCC00'}[state]
    dots = f'<span style="display:block;width:2px;height:2px;background:{T["textMuted"] if state=="idle" else T["onAccent"]};box-shadow:0 5px 0 {T["textMuted"] if state=="idle" else T["onAccent"]},0 -5px 0 {T["textMuted"] if state=="idle" else T["onAccent"]}"></span>'
    pane = f'background:{T["document"]};width:100px;height:110px;{bevel(T,True) if st=="bevel" else ""}'
    if vertical:
        return (f'<div style="display:flex;align-items:stretch;height:110px">'
                f'<div style="{pane}"></div>'
                f'<div style="width:8px;background:{handle};display:flex;align-items:center;justify-content:center">{dots}</div>'
                f'<div style="{pane}"></div></div>')
    return (f'<div style="display:flex;flex-direction:column;width:220px">'
            f'<div style="{pane.replace("width:100px;height:110px","width:220px;height:52px")}"></div>'
            f'<div style="height:8px;background:{handle};display:flex;align-items:center;justify-content:center">{dots.replace("width:2px;height:2px","width:2px;height:2px")}</div>'
            f'<div style="{pane.replace("width:100px;height:110px","width:220px;height:52px")}"></div></div>')

# ---- 7. подсказка
def tooltip(T, text, w=200):
    st = T['style']
    if st == 'hud':
        body = f'<span style="display:block;padding:8px 12px;font-family:{MONO};font-size:11px;letter-spacing:.06em;text-transform:uppercase;color:{T["text"]};width:{w}px;box-sizing:border-box;{surf_raised(T)}">{text}</span>'
    else:
        body = f'<span style="display:block;padding:8px 12px;font-size:12px;line-height:1.45;color:{T["text"]};width:{w}px;box-sizing:border-box;{surf_raised(T)}">{text}</span>'
    arrow = f'<span style="position:absolute;left:{w//2-5}px;bottom:-5px;width:10px;height:10px;transform:rotate(45deg);{surf_raised(T)}"></span>'
    return f'<div style="position:relative;display:inline-block;padding-bottom:10px">{body}{arrow}</div>'

# ---- 8. хлебные крошки
def crumbs(T):
    st = T['style']
    parts = ['Система', 'Установка', 'Сеть']
    sep = f'<span style="color:{T["textMuted"]};margin:0 8px">{"/" if st=="hud" else "›"}</span>'
    if st == 'hud':
        f = f'font-family:{MONO};font-size:12px;text-transform:uppercase;letter-spacing:.08em'
    else:
        f = 'font-size:13px'
    html = ''.join(f'<span style="color:{T["textMuted"]}">{p}</span>' + sep for p in parts)
    html += f'<span style="color:{T["text"]};font-weight:700">Wi-Fi</span>'
    return f'<div style="{f};display:flex;align-items:center;height:28px">{html}</div>'

# ---- 9. сегментированный переключатель
def segmented(T, items, active):
    st = T['style']; out = ''
    for i, t in enumerate(items):
        on = i == active
        if st == 'bevel':
            css = (f"background:{'linear-gradient(#FFDD4D,#E6B800)' if on else 'linear-gradient(#46515F,#333D49)'};color:{T['onAccent'] if on else T['text']};"
                   f"{bevel(T, on)}")
        elif st == 'hud':
            css = f"background:{T['primaryFill'] if on else 'transparent'};color:{T['accent'] if on else T['textMuted']};box-shadow:inset 0 0 0 1px {T['accent'] if on else T['line']};font-family:{MONO};font-size:12px;text-transform:uppercase"
        else:
            css = f"background:{T['accent'] if on else T['control']};color:{T['onAccent'] if on else T['text']};font-weight:{700 if on else 400}"
        out += f'<span style="display:inline-flex;align-items:center;justify-content:center;height:32px;padding:0 16px;font-size:13px;box-sizing:border-box;{css}">{t}</span>'
    frame = f"display:inline-flex;gap:{'2px' if st!='hud' else '0'};padding:{'2px' if st=='bevel' else '0'};background:{T['document'] if st!='flat' else T['control']};"
    return f'<span style="{frame}">{out}</span>'

# ---- 10. чипы и бейджи
def chip(T, text, tone='neutral', removable=False):
    st = T['style']
    tones = {'neutral': (T['control'], T['text']), 'accent': (T['accent'], T['onAccent']),
             'ok': (T['success'], '#14161B'), 'warn': (T['warning'], '#14161B'),
             'err': (T['error'], '#14161B'), 'media': (T['media'], '#14161B')}
    bg, fg = tones[tone]
    if st == 'hud':
        col = {'neutral': T['textMuted'], 'accent': T['accent'], 'ok': T['success'], 'warn': T['warning'], 'err': T['error'], 'media': T['media']}[tone]
        css = f"background:transparent;color:{col};box-shadow:inset 0 0 0 1px {col};font-family:{MONO};font-size:11px;text-transform:uppercase;letter-spacing:.08em;{CH(3)}"
    elif st == 'bevel':
        css = f"background:{bg};color:{fg};{bevel(T)};font-size:12px"
    else:
        css = f"background:{bg};color:{fg};font-size:12px;{CH(3)}"
    x = f'<span style="margin-left:6px;opacity:.85">×</span>' if removable else ''
    return f'<span style="display:inline-flex;align-items:center;height:24px;padding:0 10px;box-sizing:border-box;font-weight:600;{css}">{text}{x}</span>'

def chips(T):
    row1 = ''.join(chip(T, t, k) for t, k in [('Новое', 'accent'), ('Готово', 'ok'), ('Внимание', 'warn'), ('Ошибка', 'err'), ('ИИ', 'media'), ('Черновик', 'neutral')])
    row2 = chip(T, 'Wi-Fi', 'neutral', True) + '<span style="display:inline-block;width:8px"></span>' + chip(T, 'Игры', 'accent', True)
    return f'<div style="display:flex;flex-wrap:wrap;gap:8px">{row1}</div><div style="display:flex;gap:8px;margin-top:12px">{row2}</div>'

# ---- 11. заголовок таблицы с сортировкой
def table(T):
    st = T['style']
    head_bg = T['control'] if st != 'bevel' else 'linear-gradient(#46515F,#333D49)'
    cols = [('Имя', None), ('Размер', 'asc'), ('Изменён', None)]
    cells = ''
    for name, sort in cols:
        arrow = ''
        if sort == 'asc':
            arrow = f'<svg width="9" height="9" viewBox="0 0 10 10"><path d="M2 7l3-4 3 4" fill="{T["accent"]}"/></svg>'
        cells += (f'<span style="display:flex;align-items:center;gap:6px;color:{T["text"]};font-weight:700;font-size:12px;'
                  f'{"font-family:"+MONO+";text-transform:uppercase;letter-spacing:.06em;" if st=="hud" else ""}">{name}{arrow}</span>')
    head = (f'<div style="display:grid;grid-template-columns:1fr 80px 110px;align-items:center;height:30px;padding:0 10px;box-sizing:border-box;'
            f'background:{head_bg};{bevel(T) if st=="bevel" else ""};{"border-bottom:1px solid "+T["accent"] if st=="hud" else ""}">{cells}</div>')
    rows = [('отчёт.md', '4 КБ', '12:40'), ('схема.svg', '18 КБ', 'вчера'), ('данные.csv', '2,1 МБ', '02.11')]
    body = ''
    for i, (a, b, c) in enumerate(rows):
        on = i == 1
        bg = T['selection'] if on else ('transparent' if i % 2 == 0 else T['control'] + '88')
        fg = T['onSelection'] if (on and st == 'bevel') else (T['accent'] if on and st == 'hud' else T['text'])
        body += f'<div style="display:grid;grid-template-columns:1fr 80px 110px;align-items:center;height:28px;padding:0 10px;box-sizing:border-box;font-size:13px;background:{bg};color:{fg}"><span>{a}</span><span style="color:{T["textMuted"] if not on else fg}">{b}</span><span style="color:{T["textMuted"] if not on else fg}">{c}</span></div>'
    return f'<div style="background:{T["document"]};width:100%;{bevel(T,True) if st=="bevel" else ""}">{head}{body}</div>'

# ---- 12. строка дерева
def tree(T):
    rows = [(0, 'Система', 'open', False), (1, 'Настройки', 'open', True), (2, 'Экран', 'leaf', False), (2, 'Звук', 'leaf', False),
            (1, 'Приложения', 'closed', False), (0, 'Документы', 'closed', False)]
    st = T['style']; out = ''
    for d, name, kind, on in rows:
        caret = {'open': chev(T['textMuted'], 'down', 9), 'closed': chev(T['textMuted'], 'right', 9), 'leaf': '<span style="width:9px;display:inline-block"></span>'}[kind]
        bg = T['selection'] if on else 'transparent'
        fg = T['onSelection'] if (on and st == 'bevel') else (T['accent'] if on and st == 'hud' else T['text'])
        font = f'font-family:{MONO};font-size:12px;' if st == 'hud' else 'font-size:13px;'
        out += (f'<div style="display:flex;align-items:center;gap:6px;height:26px;padding-left:{8 + d*18}px;box-sizing:border-box;background:{bg};color:{fg};{font}">'
                f'{caret}<span>{name}</span></div>')
    return f'<div style="background:{T["document"]};width:100%;padding:4px 0;box-sizing:border-box;{bevel(T,True) if st=="bevel" else ""}">{out}</div>'

# ---- 13. индикатор ожидания
def spinner(T, label='Загрузка'):
    st = T['style']; col = T['accent']
    if st == 'bevel':
        arc = f'<svg width="32" height="32" viewBox="0 0 32 32"><circle cx="16" cy="16" r="11" fill="none" stroke="#333D49" stroke-width="5"/><circle cx="16" cy="16" r="11" fill="none" stroke="{col}" stroke-width="5" stroke-dasharray="30 70" transform="rotate(-90 16 16)"/></svg>'
    elif st == 'hud':
        arc = f'<svg width="32" height="32" viewBox="0 0 32 32"><rect x="4" y="4" width="24" height="24" fill="none" stroke="{T["line"]}" stroke-width="2"/><path d="M4 4h10" stroke="{col}" stroke-width="3"/></svg>'
    else:
        arc = f'<svg width="32" height="32" viewBox="0 0 32 32"><circle cx="16" cy="16" r="11" fill="none" stroke="{T["line"]}" stroke-width="4"/><path d="M16 5a11 11 0 0 1 11 11" fill="none" stroke="{col}" stroke-width="4"/></svg>'
    f = f'font-family:{MONO};font-size:11px;text-transform:uppercase;letter-spacing:.08em' if st == 'hud' else 'font-size:13px'
    return f'<div style="display:flex;align-items:center;gap:12px;color:{T["text"]}">{arc}<span style="{f}">{label}…</span></div>'

# ---- 14. цвет: ряд свотчей и попап
def swatches(T):
    cols = [T['accent'], T['focus'], T['success'], T['error'], T['media'], T['warning'], T['text'], T['textMuted']]
    out = ''
    for i, c in enumerate(cols):
        ring = f'box-shadow:0 0 0 2px {T["panel"]},0 0 0 4px {T["focus"]};' if i == 1 else ''
        out += f'<span style="display:inline-block;width:24px;height:24px;background:{c};{ring}{CH(3) if T["style"]=="flat" else ""}"></span>'
    row = f'<div style="display:flex;gap:8px;align-items:center">{out}</div>'
    hexv = f'<span style="font-family:{MONO};font-size:12px;color:{T["textMuted"]}">{T["focus"]}</span>'
    return f'<div>{row}<div style="margin-top:10px;display:flex;justify-content:space-between;align-items:center;width:250px">{hexv}{lab(T,"цвет выбран")}</div></div>'

# ---- 15. текст: выделение и каретка
def sel_field(T, w=280):
    st = T['style']
    caret = f'<span style="display:inline-block;width:2px;height:18px;background:{T["accent"] if st!="bevel" else "#1A1A1A"};margin-left:0"></span>'
    inner = (f'<span>Открыть</span><span style="background:{T["selection"]};color:{T["onSelection"]}">файл</span>'
             f'<span style="display:inline-block;width:2px"></span>{caret}')
    return f'<span style="{box_css(T, w)}">{inner}</span>'

def caret_states(T):
    return sel_field(T) + f'<div style="margin-top:10px">{lab(T, "выделение и каретка")}</div>'

# ---- 16. ряд кнопок диалога
def dialog_row(T):
    return (f'<div style="display:flex;justify-content:flex-end;gap:12px;align-items:center">'
            f'{button(T,"Отмена","secondary")}{button(T,"Удалить","danger")}{button(T,"Сохранить","primary")}</div>'
            + lab(T, 'отмена · опасное · основное'))

def card(T, title, body, span=1):
    st = T['style']
    bg = T['panel']
    return (f'<div style="grid-column:span {span};background:{bg};padding:18px 20px;box-sizing:border-box;display:flex;flex-direction:column;gap:14px;'
            f'{bevel(T) if st=="bevel" else ""}">'
            f'<div style="font-family:{MONO};font-size:11px;letter-spacing:.08em;text-transform:uppercase;color:{T["textMuted"]}">{title}</div>'
            f'<div>{body}</div></div>')

def board(key):
    T = THEMES[key]
    st = T['style']
    cards = [
        card(T, '01 · Радиокнопка', radios(T)),
        card(T, '02 · Выпадающий список', select(T, 'Обычный', False) + '<div style="height:10px"></div>' + select(T, 'Крупный', True)),
        card(T, '03 · Комбобокс с подсказками', combo(T)),
        card(T, '04 · Степпер числа', stepper(T) + lab(T, 'с кнопками ▲ ▼')),
        card(T, '05 · Полоса прокрутки', scrollbars(T)),
        card(T, '06 · Разделитель панелей', splitter(T, 'idle') + '<div style="height:12px"></div>' + splitter(T, 'hover', False) + lab(T, 'горизонтальный (ниже)')),
        card(T, '07 · Подсказка', tooltip(T, 'Скопировать путь в буфер обмена')),
        card(T, '08 · Хлебные крошки', crumbs(T) + lab(T, 'путь к текущему месту')),
        card(T, '09 · Сегментированный переключатель', segmented(T, ['Все', 'Игры', 'Медиа'], 1)),
        card(T, '10 · Чипы и бейджи', chips(T)),
        card(T, '11 · Заголовок таблицы', table(T), span=2),
        card(T, '12 · Строка дерева', tree(T)),
        card(T, '13 · Индикатор ожидания', spinner(T)),
        card(T, '14 · Цвет и свотч', swatches(T)),
        card(T, '15 · Выделение текста и каретка', caret_states(T)),
        card(T, '16 · Ряд кнопок диалога', dialog_row(T), span=2),
    ]
    head = (f'<div style="grid-column:span 3;display:flex;justify-content:space-between;align-items:flex-end;padding:0 4px 4px">'
            f'<div><div style="font-size:20px;font-weight:700;color:{"#0E1A2B" if st=="bevel" else T["text"]}">Недостающие компоненты · {T["name"]}</div>'
            f'<div style="font-size:13px;color:{"#1B2A3F" if st=="bevel" else T["textMuted"]};margin-top:4px">{T["role"]} · срез {T["chamfer"][0]}/{T["chamfer"][1]}/{T["chamfer"][2]} · движение {T["motion"][0]}/{T["motion"][1]}/{T["motion"][2]} мс</div></div></div>')
    grid = (f'<div style="display:grid;grid-template-columns:repeat(3,1fr);gap:1px;background:{T["line"]}">'
            + ''.join(cards) + '</div>')
    css = (f'<style>@import url("https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;700&family=Onest:wght@400;600;700&display=swap");'
           f'body{{margin:0}}</style>')
    return (css + f'<div style="width:1280px;padding:24px;box-sizing:border-box;background:{T["desktop"]};font-family:{SANS};color:{T["text"]}">'
            + head + '<div style="height:16px"></div>' + grid + '</div>')

BOARDS = [('ComponentsIndustrial', 'industrial-dark'), ('ComponentsNightOps', 'night-ops'), ('ComponentsZeta', 'zeta-remastered')]
if __name__ == '__main__':
    for fn, key in BOARDS:
        html = board(key)
        open(fn + '.dc.html', 'w').write(page('Недостающие компоненты · ' + THEMES[key]['name'], 1280, 1400, html))
        open(fn + '_prev.html', 'w').write('<!doctype html><meta charset=utf-8><style>body{margin:0}</style>' + html)
    print('ok')
