package app.vey;

import android.app.Activity;
import android.app.Dialog;
import android.graphics.SurfaceTexture;
import android.media.MediaPlayer;
import android.media.PlaybackParams;
import android.net.Uri;
import android.view.Surface;
import android.view.TextureView;
import android.view.View;
import android.view.ViewGroup;
import android.webkit.JavascriptInterface;
import android.webkit.WebResourceRequest;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.widget.FrameLayout;
import org.json.JSONObject;
import java.util.concurrent.ConcurrentLinkedQueue;

/** Small platform surfaces. Rust owns credentials, requests, audio and lyric parsing. */
public final class VeyBridge {
    private static final ConcurrentLinkedQueue<String> messages = new ConcurrentLinkedQueue<>();
    private static WebView auth;
    private static Dialog dialog;
    private static java.lang.ref.WeakReference<View> authReturnFocus = new java.lang.ref.WeakReference<>(null);
    private static volatile int authSession;
    private static TextureView texture;
    private static MediaPlayer player;
    private static Surface surface;
    private static JSONObject videoState;
    private static String videoUrl = "";
    private static boolean prepared, failed;
    private static boolean windowVisible=true;

    public static String poll() { String value=messages.poll(); return value==null ? "" : value; }
    public static void call(Activity activity, String method, String payload) {
        try {
            if (method.equals("challenge")) challenge(activity,new JSONObject(payload));
            else if (method.equals("cancelChallenge")) closeAuth();
            else if (method.equals("video")) video(activity,new JSONObject(payload));
            else if (method.equals("shutdown")) { closeAuth(); closeVideo(); }
        } catch (Exception e) {
            if (method.equals("challenge")) messages.offer("{\"type\":\"turnstile-error\",\"error\":\"android-host\"}");
            else { failed=true; if(texture!=null)texture.setVisibility(View.INVISIBLE); }
        }
    }
    private static boolean allowed(Uri uri) {
        return "https".equals(uri.getScheme()) && ("lyrics.api.dacubeking.com".equals(uri.getHost()) || "challenges.cloudflare.com".equals(uri.getHost()));
    }
    private static void challenge(Activity activity,JSONObject request) throws Exception {
        closeAuth(); messages.clear();
        final int session=authSession;
        authReturnFocus=new java.lang.ref.WeakReference<>(activity.getCurrentFocus());
        auth=new WebView(activity);
        WebSettings settings=auth.getSettings();
        settings.setJavaScriptEnabled(true); settings.setDomStorageEnabled(true);
        settings.setAllowFileAccess(false); settings.setAllowContentAccess(false);
        settings.setMixedContentMode(WebSettings.MIXED_CONTENT_NEVER_ALLOW);
        auth.setWebViewClient(new WebViewClient(){
            @Override public boolean shouldOverrideUrlLoading(WebView view,WebResourceRequest request){return !allowed(request.getUrl());}
        });
        auth.addJavascriptInterface(new Object(){
            @JavascriptInterface public void postMessage(String raw) {
                if(session!=authSession)return;
                if(raw==null||raw.length()>17000)return;
                try {
                    JSONObject value=new JSONObject(raw);
                    String type=value.optString("type");
                    if(type.equals("turnstile-token")) {
                        String token=value.optString("token").trim();
                        if(token.isEmpty()||token.length()>=16384||token.contains("\n")||token.contains("\r"))return;
                        messages.offer(raw); activity.runOnUiThread(()->closeAuth());
                    } else if(type.equals("turnstile-error")||type.equals("turnstile-expired")||type.equals("turnstile-timeout"))messages.offer(raw);
                } catch(Exception ignored) {}
            }
        },"VeyAuth");
        if(request.optBoolean("silent")) {
            float density=activity.getResources().getDisplayMetrics().density;
            FrameLayout.LayoutParams params=new FrameLayout.LayoutParams((int)(480*density),(int)(360*density));
            auth.setTranslationX(-10000); auth.setAlpha(0.01f);
            auth.setFocusable(false); auth.setFocusableInTouchMode(false);
            auth.setDescendantFocusability(ViewGroup.FOCUS_BLOCK_DESCENDANTS);
            auth.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS);
            ((ViewGroup)activity.findViewById(android.R.id.content)).addView(auth,params);
        } else {
            dialog=new Dialog(activity); dialog.setContentView(auth);
            dialog.setOnCancelListener(d->{messages.offer("{\"type\":\"turnstile-error\",\"error\":\"cancelled\"}");closeAuth();});
            dialog.show(); dialog.getWindow().setLayout((int)(activity.getResources().getDisplayMetrics().widthPixels*0.9),Math.min((int)(420*activity.getResources().getDisplayMetrics().density),(int)(activity.getResources().getDisplayMetrics().heightPixels*0.8)));
        }
        String bootstrap="<script>window.ipc={postMessage:function(s){VeyAuth.postMessage(s)}};</script>";
        auth.loadDataWithBaseURL("https://lyrics.api.dacubeking.com/",bootstrap+request.getString("html"),"text/html","UTF-8",null);
    }
    private static void closeAuth() {
        final boolean restore=auth!=null&&auth.hasFocus();
        final View previous=authReturnFocus.get();
        authReturnFocus.clear();
        authSession++;
        if(dialog!=null){dialog.dismiss();dialog=null;}
        if(auth!=null){if(auth.getParent() instanceof ViewGroup)((ViewGroup)auth.getParent()).removeView(auth);auth.removeJavascriptInterface("VeyAuth");auth.stopLoading();auth.destroy();auth=null;}
        if(restore&&previous!=null)previous.post(()->{
            View current=previous.getRootView().findFocus();
            if(previous.isAttachedToWindow()&&previous.hasWindowFocus()&&(current==null||current==previous||current==previous.getRootView()))previous.requestFocus();
        });
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
