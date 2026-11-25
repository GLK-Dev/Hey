from playwright.sync_api import sync_playwright
import os
names=['ComponentsIndustrial','ComponentsNightOps','ComponentsZeta']
with sync_playwright() as p:
    b=p.chromium.launch(executable_path='/usr/bin/google-chrome',args=['--no-sandbox','--allow-file-access-from-files'])
    pg=b.new_page(viewport={'width':1280,'height':1000})
    for n in names:
        pg.goto('file://'+os.path.abspath(n+'_prev.html')); pg.wait_for_timeout(1200)
        pg.evaluate("()=>document.getAnimations().forEach(a=>a.pause())")
        pg.screenshot(path=n+'.png',full_page=True)
    b.close()
