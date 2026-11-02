// Executed only inside the Vey-owned profile's https://music.youtube.com tab.
// Authentication remains in that tab. Return only normalized account/catalog data.
async function veyYouTube(request) {
  const known = new Set(['wrong_origin','sign_in_required','session_unavailable','account_changed','invalid_request','invalid_response','unknown_row','page_cycle','too_large','rate_limited','access_denied','request_failed']);
  const fail = code => {throw new Error(code);};
  try {
    if (location.origin !== 'https://music.youtube.com') fail('wrong_origin');
    if (!['identity','playlists','playlist'].includes(request.operation)) fail('invalid_request');
    const cfg = key => globalThis.ytcfg?.get?.(key);
    if (cfg('LOGGED_IN') === false) fail('sign_in_required');
    const context = JSON.parse(JSON.stringify(cfg('INNERTUBE_CONTEXT') || {}));
    if (!context.client?.clientVersion) fail('session_unavailable');
    const cookies = new Map(document.cookie.split(';').map(s => {
      const i=s.indexOf('=');return [s.slice(0,i).trim(),s.slice(i+1)];
    }));
    const sid = cookies.get('SAPISID') || cookies.get('__Secure-3PAPISID');
    if (!sid) fail('sign_in_required');
    const authuser = String(cfg('SESSION_INDEX') || 0);
    const apiKey = cfg('INNERTUBE_API_KEY');
    const text = node => typeof node?.simpleText === 'string' ? node.simpleText : (node?.runs || []).map(r=>r.text || '').join('');
    const walk = (node, callback, depth=0) => {
      if (!node || typeof node !== 'object' || depth > 40) return;
      callback(node);
      for (const value of Object.values(node)) walk(value, callback, depth+1);
    };
    const find = (node, keys) => {
      let result;
      walk(node,n=>{if(result)return;for(const key of keys)if(n[key]){result=n[key];break;}});
      return result;
    };
    const validVideo = id => typeof id === 'string' && /^[A-Za-z0-9_-]{11}$/.test(id);
    const validPlaylist = id => typeof id === 'string' && (id === 'LM' || /^[A-Za-z0-9_-]{10,63}$/.test(id));
    const api = async (endpoint, body) => {
      // Only read operations are reachable. Do not return sid, auth or raw headers.
      if (!['browse','account/account_menu'].includes(endpoint)) fail('invalid_request');
      const seconds = String(Math.floor(Date.now()/1000));
      const digest = await crypto.subtle.digest('SHA-1', new TextEncoder().encode(`${seconds} ${sid} https://music.youtube.com`));
      const hash = Array.from(new Uint8Array(digest),b=>b.toString(16).padStart(2,'0')).join('');
      const headers = {
        'content-type':'application/json',
        'authorization':`SAPISIDHASH ${seconds}_${hash}`,
        'x-goog-authuser':authuser,
        'x-youtube-client-name':String(cfg('INNERTUBE_CONTEXT_CLIENT_NAME') || 67),
        'x-youtube-client-version':context.client.clientVersion,
        'x-origin':'https://music.youtube.com'
      };
      if(context.client.visitorData)headers['x-goog-visitor-id']=context.client.visitorData;
      const response = await fetch(`https://music.youtube.com/youtubei/v1/${endpoint}?prettyPrint=false${apiKey ? '&key='+encodeURIComponent(apiKey) : ''}`, {
        method:'POST',credentials:'include',headers,body:JSON.stringify({...body,context}),signal:AbortSignal.timeout(25000)
      });
      if(response.status===401)fail('sign_in_required');
      if(response.status===403)fail('access_denied');
      if(response.status===429)fail('rate_limited');
      if(!response.ok)fail('request_failed');
      return response.json();
    };
    const menu = await api('account/account_menu',{});
    const account = find(menu,['activeAccountHeaderRenderer']);
    const label = text(account?.accountName).trim();
    const owner = String(cfg('DATASYNC_ID') || text(account?.channelHandle) || '').trim();
    if(!label || !owner)fail('sign_in_required');
    const identity = {owner:owner.slice(0,512),label:label.slice(0,160)};
    if(request.expectedOwner && request.expectedOwner!==identity.owner)fail('account_changed');
    if(request.operation==='identity')return {ok:true,identity};
    const shelf = data => find(data,request.operation==='playlists'
      ? ['gridRenderer','musicGridRenderer','gridContinuation','musicGridContinuation','musicShelfContinuation']
      : ['musicPlaylistShelfRenderer','musicPlaylistShelfContinuation','musicShelfContinuation']) || find(data,['appendContinuationItemsAction']);
    const pageItems = node => node?.items || node?.contents || node?.continuationItems;
    const next = node => {
      let token=node?.continuations?.[0]?.nextContinuationData?.continuation;
      for(const item of pageItems(node)||[])token=item.continuationItemRenderer?.continuationEndpoint?.continuationCommand?.token || token;
      return typeof token==='string' && token ? token : null;
    };
    let token, title='', tracks=[], playlists=[];
    const tokens=new Set(), playlistIds=new Set();
    if(request.operation==='playlist' && !validPlaylist(request.id))fail('invalid_request');
    for(let page=0;page<400;page++) {
      const body=token ? {continuation:token} : {browseId:request.operation==='playlists' ? 'FEmusic_liked_playlists' : 'VL'+request.id};
      const data=await api('browse',body);
      const node=shelf(data);
      const items=pageItems(node);
      if(!Array.isArray(items))fail('invalid_response');
      if(!title && request.operation==='playlist')title=text(find(data,['musicResponsiveHeaderRenderer','musicDetailHeaderRenderer'])?.title);
      for(const item of items) {
        if(item.continuationItemRenderer)continue;
        if(request.operation==='playlists') {
          const tile=item.musicTwoRowItemRenderer || item.musicResponsiveListItemRenderer;
          if(!tile)fail('unknown_row');
          const browse=tile.navigationEndpoint?.browseEndpoint?.browseId || tile.title?.runs?.find(r=>r.navigationEndpoint?.browseEndpoint)?.navigationEndpoint?.browseEndpoint?.browseId;
          // Ignore the create-playlist tile and non-playlist media, never infer IDs.
          if(!browse?.startsWith('VL'))continue;
          const id=browse.slice(2);
          if(!validPlaylist(id))fail('unknown_row');
          if(playlistIds.has(id))continue;
          playlistIds.add(id);
          playlists.push({id,name:(text(tile.title)||text(tile.flexColumns?.[0]?.musicResponsiveListItemFlexColumnRenderer?.text)||id).slice(0,160)});
          if(playlists.length>500)fail('too_large');
        } else {
          const row=item.musicResponsiveListItemRenderer;
          if(!row)fail('unknown_row');
          const endpoint=find(row,['watchEndpoint']);
          const id=row.playlistItemData?.videoId || endpoint?.videoId;
          if(!validVideo(id))fail('unknown_row');
          const column = i => row.flexColumns?.[i]?.musicResponsiveListItemFlexColumnRenderer?.text;
          const title=text(column(0)) || 'Unavailable track';
          const byline=column(1)?.runs || [];
          const artist=byline.filter(r=>r.navigationEndpoint?.browseEndpoint?.browseId?.startsWith('UC')).map(r=>r.text).join(' · ') || text(column(1));
          const album=text(column(2));
          const clock=text(row.fixedColumns?.[0]?.musicResponsiveListItemFixedColumnRenderer?.text);
          const duration=clock.split(':').reduce((sum,v)=>sum*60+(Number(v)||0),0);
          const thumbnails=find(row,['musicThumbnailRenderer'])?.thumbnail?.thumbnails || [];
          tracks.push({path:'yt://'+id,videoId:id,title:title.slice(0,512),artist:artist.slice(0,512),album:album.slice(0,512),durationSecs:Math.min(duration,604800),thumbnailUrl:thumbnails.at(-1)?.url || ''});
          if(tracks.length>20000)fail('too_large');
        }
      }
      token=next(node);
      if(!token)return request.operation==='playlists' ? {ok:true,identity,playlists} : {ok:true,identity,name:title||request.id,tracks};
      if(tokens.has(token))fail('page_cycle');
      tokens.add(token);
    }
    fail('too_large');
  } catch(error) {
    return {ok:false,error:known.has(error?.message)?error.message:'request_failed'};
  }
}
