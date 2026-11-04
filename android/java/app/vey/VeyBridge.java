package app.vey;

import android.app.Activity;
import android.graphics.SurfaceTexture;
import android.media.MediaPlayer;
import android.media.MediaMetadata;
import android.media.session.MediaSession;
import android.media.session.PlaybackState;
import android.media.PlaybackParams;
import android.net.Uri;
import android.view.Surface;
import android.view.TextureView;
import android.view.View;
import android.view.ViewGroup;
import android.widget.FrameLayout;
import org.json.JSONObject;
import java.util.concurrent.ConcurrentLinkedQueue;

/** Small platform surfaces. Rust owns credentials, requests, audio and lyric parsing. */
public final class VeyBridge {
    private static final ConcurrentLinkedQueue<String> messages = new ConcurrentLinkedQueue<>();
    private static final ConcurrentLinkedQueue<String> mediaMessages = new ConcurrentLinkedQueue<>();
    private static TextureView texture;
    private static MediaPlayer player;
    private static Surface surface;
    private static JSONObject videoState;
    private static String videoUrl = "";
    private static boolean prepared, failed;
    private static boolean windowVisible=true;
    private static MediaSession mediaSession;

    public static String poll() { String value=messages.poll(); return value==null ? "" : value; }
    public static String mediaPoll() { String value=mediaMessages.poll(); return value==null ? "" : value; }
    public static void call(Activity activity, String method, String payload) {
        try {
            if (method.equals("video")) video(activity,new JSONObject(payload));
            else if (method.equals("media")) media(activity,new JSONObject(payload));
            else if (method.equals("shutdown")) { closeVideo(); if(mediaSession!=null){mediaSession.setActive(false);mediaSession.release();mediaSession=null;} }
        } catch (Exception e) {
            failed=true; if(texture!=null)texture.setVisibility(View.INVISIBLE);
        }
    }
    private static void media(Activity activity, JSONObject data) {
        if(mediaSession==null) {
            mediaSession=new MediaSession(activity,"Vey");
            mediaSession.setFlags(MediaSession.FLAG_HANDLES_MEDIA_BUTTONS|MediaSession.FLAG_HANDLES_TRANSPORT_CONTROLS);
            mediaSession.setCallback(new MediaSession.Callback(){
                private void post(String action){mediaMessages.offer("{\"action\":\""+action+"\"}");}
                @Override public void onPlay(){post("play");}
                @Override public void onPause(){post("pause");}
                @Override public void onSkipToNext(){post("next");}
                @Override public void onSkipToPrevious(){post("previous");}
                @Override public void onSeekTo(long position){mediaMessages.offer("{\"action\":\"seek\",\"position\":"+Math.max(0,position)+"}");}
            });
        }
        String title=data.optString("title","Vey"), artist=data.optString("artist"), album=data.optString("album");
        long duration=Math.max(0,(long)(data.optDouble("duration",0)*1000));
        long position=Math.max(0,(long)(data.optDouble("position",0)*1000));
        boolean playing=data.optBoolean("playing",false), active=data.optBoolean("active",false);
        mediaSession.setMetadata(new MediaMetadata.Builder().putString(MediaMetadata.METADATA_KEY_TITLE,title)
            .putString(MediaMetadata.METADATA_KEY_ARTIST,artist).putString(MediaMetadata.METADATA_KEY_ALBUM,album)
            .putLong(MediaMetadata.METADATA_KEY_DURATION,duration).build());
        long actions=PlaybackState.ACTION_PLAY|PlaybackState.ACTION_PAUSE|PlaybackState.ACTION_PLAY_PAUSE
            |PlaybackState.ACTION_SKIP_TO_NEXT|PlaybackState.ACTION_SKIP_TO_PREVIOUS|PlaybackState.ACTION_SEEK_TO;
        mediaSession.setPlaybackState(new PlaybackState.Builder().setActions(actions)
            .setState(playing?PlaybackState.STATE_PLAYING:(active?PlaybackState.STATE_PAUSED:PlaybackState.STATE_STOPPED),position,1f)
            .build());
        mediaSession.setActive(active);
    }
    private static void closeVideo() {
        prepared=false;
        if(player!=null){player.release();player=null;}
        if(surface!=null){surface.release();surface=null;}
        if(texture!=null){if(texture.getParent() instanceof ViewGroup)((ViewGroup)texture.getParent()).removeView(texture);texture=null;}
        videoUrl="";
    }
    private static void prepare(Activity activity) {
        if(texture==null||!texture.isAvailable()||videoUrl.isEmpty()||failed||player!=null||videoState==null||!videoState.optBoolean("visible")||!windowVisible)return;
        try {
            player=new MediaPlayer(); surface=new Surface(texture.getSurfaceTexture());
            player.setSurface(surface);player.setVolume(0,0);
            player.setOnPreparedListener(p->{prepared=true;sync();});
            player.setOnErrorListener((p,what,extra)->{failed=true;prepared=false;p.release();player=null;if(surface!=null){surface.release();surface=null;}if(texture!=null)texture.setVisibility(View.INVISIBLE);return true;});
            player.setDataSource(activity,Uri.parse(videoUrl)); player.prepareAsync();
        }catch(Exception e){failed=true;texture.setVisibility(View.INVISIBLE);}
    }
    private static void video(Activity activity,JSONObject state) throws Exception {
        videoState=state;
        String url=state.optString("url");
        if(!url.equals(videoUrl)) {
            closeVideo(); videoUrl=url;failed=false;
            if(url.isEmpty()||!"https".equals(Uri.parse(url).getScheme()))return;
            texture=new TextureView(activity){
                @Override protected void onWindowVisibilityChanged(int visibility){super.onWindowVisibilityChanged(visibility);windowVisible=visibility==View.VISIBLE;if(!windowVisible&&prepared&&player!=null)try{player.pause();}catch(Exception ignored){}}
            };
            texture.setOpaque(false); texture.setFocusable(false);texture.setClickable(false);
            texture.setSurfaceTextureListener(new TextureView.SurfaceTextureListener(){
                public void onSurfaceTextureAvailable(SurfaceTexture t,int w,int h){prepare(activity);}
                public void onSurfaceTextureSizeChanged(SurfaceTexture t,int w,int h){}
                public boolean onSurfaceTextureDestroyed(SurfaceTexture t){if(player!=null){player.release();player=null;}if(surface!=null){surface.release();surface=null;}prepared=false;return true;}
                public void onSurfaceTextureUpdated(SurfaceTexture t){}
            });
            ((ViewGroup)activity.findViewById(android.R.id.content)).addView(texture,new FrameLayout.LayoutParams(1,1));
        }
        if(texture==null)return;
        JSONObject b=state.getJSONObject("bounds");
        FrameLayout.LayoutParams params=new FrameLayout.LayoutParams(Math.max(1,b.optInt("width")),Math.max(1,b.optInt("height")));
        params.leftMargin=b.optInt("x");params.topMargin=b.optInt("y");texture.setLayoutParams(params);
        boolean visible=state.optBoolean("visible")&&!failed&&windowVisible;
        // A visible transparent TextureView must create its SurfaceTexture before prepare.
        texture.setVisibility(visible?View.VISIBLE:View.INVISIBLE);
        texture.setAlpha(prepared?1f:0f);
        if(visible&&player==null&&texture.isAvailable())prepare(activity);
        sync();
    }
    private static void sync() {
        if(!prepared||player==null||videoState==null)return;
        try {
            boolean playing=videoState.optBoolean("playing")&&videoState.optBoolean("visible")&&windowVisible;
            if(player.getDuration()>0 && videoState.optDouble("position")*1000>=player.getDuration()){
                if(player.isPlaying())player.pause();if(texture!=null)texture.setVisibility(View.INVISIBLE);return;
            }
            int target=(int)Math.max(0,Math.min(player.getDuration(),videoState.optDouble("position")*1000));
            if(Math.abs(player.getCurrentPosition()-target)>(playing?250:40))player.seekTo(target,MediaPlayer.SEEK_CLOSEST);
            float rate=(float)Math.max(0.25,Math.min(4,videoState.optDouble("rate",1)));
            if(playing){player.setPlaybackParams(new PlaybackParams().setSpeed(rate));if(!player.isPlaying())player.start();}
            else if(player.isPlaying())player.pause();
            player.setVolume(0,0);
            if(texture!=null&&videoState.optBoolean("visible")&&windowVisible){
                int vw=player.getVideoWidth(),vh=player.getVideoHeight();
                float width=texture.getWidth(),height=texture.getHeight();
                if(vw>0&&vh>0&&width>0&&height>0){
                    float scale=Math.min(width/vw,height/vh);
                    android.graphics.Matrix matrix=new android.graphics.Matrix();
                    matrix.setScale(vw*scale/width,vh*scale/height,width/2,height/2);texture.setTransform(matrix);
                }
                texture.setAlpha(1);texture.setVisibility(View.VISIBLE);
            }
        }catch(Exception e){failed=true;if(texture!=null)texture.setVisibility(View.INVISIBLE);}
    }
}
