import { test, expect, type Page } from '@playwright/test';

async function workspace(page:Page,mode:'normal'|'hold'|'error'='normal') {
 await page.addInitScript(mode=>{
  const w=window as any;w.isTauri=true;let callback=0;
  const callbacks=new Map(),listeners=new Map();
  const fixture=w.newFocusFixture={calls:[] as any[],live:[] as any[],release:()=>{},pending:false};
  w.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener(){}};
  w.__TAURI_INTERNALS__={transformCallback(handler:any){callbacks.set(++callback,handler);return callback;},unregisterCallback(){},async invoke(command:string,args:any={}){
   fixture.calls.push({command,args});
   if(command==='plugin:event|listen'){listeners.set(args.event,callbacks.get(args.handler));return callback;}
   if(command.startsWith('plugin:event|'))return null;
   if(command==='get_ui_language')return 'ko';
   if(command==='load_layout')return{version:1,columns:1,sessions:[]};
   if(command==='list_sessions')return fixture.live;
   if(command==='list_projects')return['alpha','beta'].map(id=>({id,name:id,cwd:'D:\\'+id,createdAt:1,favorite:false}));
   if(command==='get_codex_settings')return{savedPath:null,resolvedPath:'D:\\own-fixture\\codex.exe',source:'environment',error:null};
   if(command==='start_session'){
    fixture.pending=true;if(mode==='hold')await new Promise<void>(resolve=>{fixture.release=resolve;});
    if(mode==='error')throw Error('OWN_START_REJECTED');
    const info={...args.request,id:'own-created',generation:1,threadId:'01900000-0000-7000-8000-000000000001',state:'idle',active:true};
    fixture.live.push(info);listeners.get('session-changed')?.({payload:info});return info;
   }
   if(command==='conversation_request'&&args.request.type==='contextStatus')return{connected:true,guiConnected:false,passiveSubscriptionSupported:true,revision:1,active:false};
   return null;
  }};
 },mode);
 await page.goto('/');await expect(page.locator('#app')).toHaveAttribute('aria-busy','false');
 await page.locator('#new-session').click();await expect(page.locator('#create-dialog')).toBeVisible();
 await page.locator('#session-title').fill('OWN_NEW_INPUT');await page.locator('#session-cwd').fill('D:\\alpha');
}

test('CREATE-FIRST-INPUT new modal completion accepts first key without another click',async({page},testInfo)=>{
 await workspace(page);await page.locator('#create-submit').click();await expect(page.locator('#create-dialog')).not.toBeVisible();
 const terminal=page.locator('.terminal-card[data-id="own-created"] .xterm-helper-textarea');await expect(terminal).toHaveCount(1);
 const before=await page.evaluate(()=>({active:document.activeElement?.outerHTML,modal:document.querySelector('#create-dialog')?.matches(':modal')}));
 await page.keyboard.press('z');
 const writes=()=>page.evaluate(()=>(window as any).newFocusFixture.calls.filter((c:any)=>c.command==='session_write').map((c:any)=>c.args));
 await testInfo.attach('first-key-observation',{body:JSON.stringify({before,writes:await writes()}),contentType:'application/json'});
 await expect.soft(terminal).toBeFocused();await expect.soft.poll(writes).toEqual([{id:'own-created',generation:1,data:'z'}]);
});

test('CREATE-LATE-PROJECT delayed completion preserves newer project selection and focus',async({page})=>{
 await workspace(page,'hold');await page.locator('#create-submit').click();await expect.poll(()=>page.evaluate(()=>(window as any).newFocusFixture.pending)).toBe(true);
 await page.locator('#create-dialog').press('Escape');const beta=page.locator('#project-tabs [data-project-key="id:beta"]');await beta.click();
 await page.evaluate(()=>(window as any).newFocusFixture.release());await expect(page.locator('.terminal-card[data-id="own-created"]')).toHaveCount(1);
 await expect(beta).toHaveAttribute('aria-selected','true');await expect(beta).toBeFocused();await expect(page.locator('.terminal-card[data-id="own-created"]')).not.toBeVisible();
});

test('CREATE-ERROR rejected start preserves modal fields and retry',async({page})=>{
 await workspace(page,'error');await page.locator('#create-submit').click();await expect(page.locator('#create-error')).toBeVisible();
 await expect(page.locator('#create-dialog')).toBeVisible();await expect(page.locator('#session-title')).toHaveValue('OWN_NEW_INPUT');await expect(page.locator('#session-cwd')).toHaveValue('D:\\alpha');await expect(page.locator('#create-submit')).toBeEnabled();
 expect(await page.evaluate(()=>(window as any).newFocusFixture.calls.filter((c:any)=>c.command==='session_write'))).toEqual([]);
});
