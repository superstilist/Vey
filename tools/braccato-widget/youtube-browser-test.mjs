// Execute the exact browser helper with an isolated fake YouTube origin/session.
// No real browser profile, login, credentials or live account endpoints are used.
import {readFile} from 'node:fs/promises';
import vm from 'node:vm';
import {webcrypto} from 'node:crypto';
import assert from 'node:assert/strict';
const source=await readFile(new URL('../../assets/youtube-browser.js',import.meta.url),'utf8');
const header={accountName:{runs:[{text:'Fixture Account'}]},channelHandle:{runs:[{text:'@fixture'}]}};
const menu={actions:[{openPopupAction:{popup:{multiPageMenuRenderer:{header:{activeAccountHeaderRenderer:header}}}}}]};
const row=id=>({musicResponsiveListItemRenderer:{playlistItemData:{videoId:id},flexColumns:[{musicResponsiveListItemFlexColumnRenderer:{text:{runs:[{text:'A song'}]}}},{musicResponsiveListItemFlexColumnRenderer:{text:{runs:[{text:'Artist',navigationEndpoint:{browseEndpoint:{browseId:'UCfixture'}}}]}}}],fixedColumns:[{musicResponsiveListItemFixedColumnRenderer:{text:{simpleText:'3:12'}}}]}});
const continuation=token=>({continuationItemRenderer:{continuationEndpoint:{continuationCommand:{token}}}});
const fixture=(responses,origin='https://music.youtube.com')=>{
  const requests=[];let index=0;
  const cfg={LOGGED_IN:true,DATASYNC_ID:'fixture-owner',SESSION_INDEX:0,INNERTUBE_CONTEXT:{client:{clientVersion:'1.fixture',clientName:'WEB_REMIX'}}};
  const context={location:{origin},document:{cookie:'SAPISID=fake-test-only-secret'},ytcfg:{get:key=>cfg[key]},crypto:webcrypto,TextEncoder,AbortSignal,URL,fetch:async(url,options)=>{
    assert.equal(new URL(url).origin,'https://music.youtube.com');assert.equal(options.credentials,'include');assert.equal(options.method,'POST');
    assert.match(options.headers.authorization,/^SAPISIDHASH \d+_[a-f0-9]{40}$/);assert.ok(!options.headers.authorization.includes('fake-test-only-secret'));
    requests.push({url,body:JSON.parse(options.body)});
    return {ok:true,status:200,json:async()=>responses[index++]};
  }};
  return {run:vm.runInNewContext(`(${source})`,context),requests,cfg,context};
};
let f=fixture([menu]);let result=await f.run({operation:'identity'});
assert.equal(result.identity.owner,'fixture-owner');assert.equal(result.identity.label,'Fixture Account');assert.ok(!JSON.stringify(result).includes('fake-test-only-secret'));
f=fixture([]);f.context.fetch=async()=>({ok:false,status:429});result=await f.run({operation:'identity'});assert.equal(result.error,'rate_limited');
f=fixture([],'https://accounts.google.com');result=await f.run({operation:'identity'});assert.equal(result.error,'wrong_origin');assert.equal(f.requests.length,0,'never inspect Google login fields or run account API on Google origin');
f=fixture([]);result=await f.run({operation:'delete_playlist'});assert.equal(result.error,'invalid_request');assert.equal(f.requests.length,0,'no write operation exists');
f=fixture([menu]);result=await f.run({operation:'playlist',id:'PL1234567890',expectedOwner:'another-owner'});assert.equal(result.error,'account_changed');assert.equal(f.requests.length,1,'account changes stop before browsing');
f=fixture([menu,{musicPlaylistShelfRenderer:{contents:[row('abcdefghijk'),row('abcdefghijk'),continuation('next')]}},{continuationContents:{musicPlaylistShelfContinuation:{contents:[row('12345678901')]}}}]);
result=await f.run({operation:'playlist',id:'PL1234567890',expectedOwner:'fixture-owner'});assert.equal(result.ok,true);assert.equal(result.tracks.length,3);assert.equal(result.tracks[0].path,result.tracks[1].path);assert.equal(result.tracks[0].durationSecs,192);assert.equal(f.requests[2].body.continuation,'next');
f=fixture([menu,{gridRenderer:{items:[{musicTwoRowItemRenderer:{title:{simpleText:'Create'}}},{musicTwoRowItemRenderer:{title:{simpleText:'Private playlist'},navigationEndpoint:{browseEndpoint:{browseId:'VLPL1234567890'}}}},{musicTwoRowItemRenderer:{title:{simpleText:'Liked songs'},navigationEndpoint:{browseEndpoint:{browseId:'VLLM'}}}}]}}]);
result=await f.run({operation:'playlists',expectedOwner:'fixture-owner'});assert.equal(result.playlists.length,2);assert.equal(result.playlists[1].id,'LM');assert.equal(f.requests[1].body.browseId,'FEmusic_liked_playlists');
f=fixture([menu,{musicPlaylistShelfRenderer:{contents:[continuation('cycle')]}},{musicPlaylistShelfContinuation:{contents:[continuation('cycle')]}}]);result=await f.run({operation:'playlist',id:'PL1234567890'});assert.equal(result.error,'page_cycle');
f=fixture([menu,{musicPlaylistShelfRenderer:{contents:[{musicResponsiveListItemRenderer:{}}]}}]);result=await f.run({operation:'playlist',id:'PL1234567890'});assert.equal(result.error,'unknown_row');
const homeTile=(title,endpoint,width=300,height=300)=>({musicTwoRowItemRenderer:{
  title:{simpleText:title},subtitle:{runs:[{text:'Artist',navigationEndpoint:{browseEndpoint:{browseId:'UCartist'}}}]},
  navigationEndpoint:endpoint,thumbnailRenderer:{musicThumbnailRenderer:{thumbnail:{thumbnails:[{url:'https://i.ytimg.com/fixture.jpg',width,height}]}}}
}});
const greyHome=homeTile('Unavailable',{watchEndpoint:{videoId:'99999999999'}});
greyHome.musicTwoRowItemRenderer.musicItemRendererDisplayPolicy='MUSIC_ITEM_RENDERER_DISPLAY_POLICY_GREY_OUT';
f=fixture([menu,{contents:[{musicCarouselShelfRenderer:{header:{musicCarouselShelfBasicHeaderRenderer:{title:{simpleText:'For you'}}},contents:[
  homeTile('Song',{watchEndpoint:{videoId:'abcdefghijk'}}),
  homeTile('Video',{watchEndpoint:{videoId:'12345678901',videoType:'MUSIC_VIDEO_TYPE_OMV'}}),
  homeTile('Album',{browseEndpoint:{browseId:'MPREalbum'}}),
  homeTile('Artist',{browseEndpoint:{browseId:'UCartist'}}),
  greyHome,
  homeTile('Song',{watchEndpoint:{videoId:'abcdefghijk'}})
]}}]}]);
result=await f.run({operation:'home',expectedOwner:'fixture-owner'});
assert.equal(result.ok,true);assert.equal(f.requests[1].body.browseId,'FEmusic_home');
assert.deepEqual(Array.from(result.catalog.sections,s=>s.kind),['song','video','album','artist']);
assert.equal(result.catalog.sections[0].items.length,1);
assert.equal(result.catalog.sections[2].items[0].browseId,'MPREalbum');
assert.ok(!JSON.stringify(result).includes('fake-test-only-secret'));
const songShelves=Array.from({length:10},(_,i)=>({musicCarouselShelfRenderer:{contents:[homeTile('Song '+i,{watchEndpoint:{videoId:String(i).padStart(11,'0')}})]}}));
f=fixture([menu,{contents:[...songShelves,{musicCarouselShelfRenderer:{contents:[homeTile('Artist',{browseEndpoint:{browseId:'UCartist'}})]}}]}]);
result=await f.run({operation:'home',expectedOwner:'fixture-owner'});
assert.equal(result.catalog.sections.length,8);assert.ok(result.catalog.sections.some(section=>section.kind==='artist'));
f=fixture([menu]);result=await f.run({operation:'home',expectedOwner:'other-owner'});
assert.equal(result.error,'account_changed');assert.equal(f.requests.length,1);
console.log('YouTube browser helper: identity, read-only API, origin/account guards, pagination, repeats and private-library parsing passed.');
