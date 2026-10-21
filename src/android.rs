//! Android shared audio access. MediaStore owns discovery; Rust owns metadata/playback.
use jni::{
    objects::{GlobalRef, JClass, JObject, JString, JValue},
    JNIEnv, JavaVM,
};
use slint::android::AndroidApp;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

static APP: Mutex<Option<AndroidApp>> = Mutex::new(None);
static PERMISSION_PENDING: AtomicBool = AtomicBool::new(false);

pub fn set_app(app: Option<AndroidApp>) {
    if app.is_none() {
        let _ = bridge_call("shutdown", "{}");
    }
    *APP.lock().unwrap() = app;
    PERMISSION_PENDING.store(false, Ordering::Relaxed);
}

static BRIDGE: std::sync::OnceLock<GlobalRef> = std::sync::OnceLock::new();
fn bridge_class(env: &mut JNIEnv, activity: &JObject) -> jni::errors::Result<GlobalRef> {
    if let Some(class) = BRIDGE.get() {
        return Ok(class.clone());
    }
    let bytes = include_bytes!(concat!(env!("OUT_DIR"), "/android-bridge/classes.dex"));
    let data = env.byte_array_from_slice(bytes)?;
    let buffer = env
        .call_static_method(
            "java/nio/ByteBuffer",
            "wrap",
            "([B)Ljava/nio/ByteBuffer;",
            &[JValue::Object(&data)],
        )?
        .l()?;
    let parent = env
        .call_method(activity, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])?
        .l()?;
    let loader = env.new_object(
        "dalvik/system/InMemoryDexClassLoader",
        "(Ljava/nio/ByteBuffer;Ljava/lang/ClassLoader;)V",
        &[JValue::Object(&buffer), JValue::Object(&parent)],
    )?;
    let name = env.new_string("app.vey.VeyBridge")?;
    let class = env
        .call_method(
            loader,
            "loadClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            &[JValue::Object(&name)],
        )?
        .l()?;
    let global = env.new_global_ref(class)?;
    let _ = BRIDGE.set(global);
    // Concurrent startup polls must use the winning loader and its one message queue.
    Ok(BRIDGE.get().unwrap().clone())
}
/// All WebView and video view operations run on Android's Java UI thread.
pub fn bridge_call(method: &str, payload: &str) -> Result<(), String> {
    let app = app()?;
    let current = app.clone();
    let method = method.to_owned();
    let payload = payload.to_owned();
    app.run_on_java_main_thread(Box::new(move || {
        let result = with_activity(&current, |env, activity| {
            let class = bridge_class(env, activity)?;
            let class: &JClass = class.as_obj().into();
            let method = env.new_string(method)?;
            let payload = env.new_string(payload)?;
            env.call_static_method(
                class,
                "call",
                "(Landroid/app/Activity;Ljava/lang/String;Ljava/lang/String;)V",
                &[
                    JValue::Object(activity),
                    JValue::Object(&method),
                    JValue::Object(&payload),
                ],
            )?;
            Ok(())
        });
        if result.is_err() {
            eprintln!("Android media/auth host could not initialize");
        }
    }));
    Ok(())
}
pub fn bridge_poll() -> Result<String, String> {
    with_activity(&app()?, |env, activity| {
        let class = bridge_class(env, activity)?;
        let class: &JClass = class.as_obj().into();
        let value = env
            .call_static_method(class, "poll", "()Ljava/lang/String;", &[])?
            .l()?;
        Ok(env.get_string(&JString::from(value))?.into())
    })
}

fn app() -> Result<AndroidApp, String> {
    APP.lock()
        .unwrap()
        .clone()
        .ok_or_else(|| "Android activity unavailable".into())
}

fn with_activity<T>(
    app: &AndroidApp,
    f: impl FnOnce(&mut JNIEnv, &JObject) -> jni::errors::Result<T>,
) -> Result<T, String> {
    // AndroidApp keeps these unowned VM/activity references valid for this call.
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) }.map_err(|e| e.to_string())?;
    let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };
    let result = env.with_local_frame(32, |env| f(env, &activity));
    if result.is_err() {
        // Do not leave a pending Java exception on a reused native/Java thread.
        let _ = env.exception_clear();
    }
    result.map_err(|e| format!("Android audio library: {e}"))
}

fn permission<'a>(env: &mut JNIEnv<'a>) -> jni::errors::Result<JString<'a>> {
    let sdk = env
        .get_static_field("android/os/Build$VERSION", "SDK_INT", "I")?
        .i()?;
    env.new_string(if sdk >= 33 {
        "android.permission.READ_MEDIA_AUDIO"
    } else {
        "android.permission.READ_EXTERNAL_STORAGE"
    })
}

pub fn has_audio_permission() -> Result<bool, String> {
    with_activity(&app()?, |env, activity| {
        let name = permission(env)?;
        Ok(env
            .call_method(
                activity,
                "checkSelfPermission",
                "(Ljava/lang/String;)I",
                &[JValue::Object(&name)],
            )?
            .i()?
            == 0)
    })
}

/// Called from a scan action. Never block the Slint thread waiting for a dialog.
pub fn ensure_audio_permission() -> Result<bool, String> {
    if has_audio_permission()? {
        return Ok(true);
    }
    if PERMISSION_PENDING.swap(true, Ordering::Relaxed) {
        return Ok(false);
    }
    let app = app()?;
    let current = app.clone();
    app.run_on_java_main_thread(Box::new(move || {
        let result = with_activity(&current, |env, activity| {
            let name = permission(env)?;
            let permissions = env.new_object_array(1, "java/lang/String", JObject::null())?;
            env.set_object_array_element(&permissions, 0, name)?;
            env.call_method(
                activity,
                "requestPermissions",
                "([Ljava/lang/String;I)V",
                &[JValue::Object(&permissions), JValue::Int(4101)],
            )?;
            Ok(())
        });
        if let Err(error) = result {
            PERMISSION_PENDING.store(false, Ordering::Relaxed);
            eprintln!("{error}");
        }
    }));
    Ok(false)
}

pub fn finish_permission_request() {
    PERMISSION_PENDING.store(false, Ordering::Relaxed);
}

pub fn has_window_focus() -> bool {
    app()
        .and_then(|app| {
            with_activity(&app, |env, activity| {
                env.call_method(activity, "hasWindowFocus", "()Z", &[])?.z()
            })
        })
        .unwrap_or(false)
}

/// Includes all indexed external audio (not just Music), including removable volumes.
/// Android 11+ permits direct file access to media granted by the audio permission.
pub fn audio_paths() -> Result<Vec<String>, String> {
    with_activity(&app()?, |env, activity| {
        let resolver = env
            .call_method(
                activity,
                "getContentResolver",
                "()Landroid/content/ContentResolver;",
                &[],
            )?
            .l()?;
        let uri = env
            .get_static_field(
                "android/provider/MediaStore$Audio$Media",
                "EXTERNAL_CONTENT_URI",
                "Landroid/net/Uri;",
            )?
            .l()?;
        let projection = env.new_object_array(1, "java/lang/String", JObject::null())?;
        let column = env.new_string("_data")?;
        env.set_object_array_element(&projection, 0, &column)?;
        let null = JObject::null();
        let cursor = env.call_method(&resolver, "query", "(Landroid/net/Uri;[Ljava/lang/String;Ljava/lang/String;[Ljava/lang/String;Ljava/lang/String;)Landroid/database/Cursor;", &[
            JValue::Object(&uri), JValue::Object(&projection), JValue::Object(&null), JValue::Object(&null), JValue::Object(&null),
        ])?.l()?;
        if cursor.is_null() {
            return Err(jni::errors::Error::NullPtr("MediaStore cursor"));
        }
        let result = (|| {
            let index = env
                .call_method(
                    &cursor,
                    "getColumnIndexOrThrow",
                    "(Ljava/lang/String;)I",
                    &[JValue::Object(&column)],
                )?
                .i()?;
            let mut paths = Vec::new();
            while env.call_method(&cursor, "moveToNext", "()Z", &[])?.z()? {
                let path = env
                    .call_method(
                        &cursor,
                        "getString",
                        "(I)Ljava/lang/String;",
                        &[JValue::Int(index)],
                    )?
                    .l()?;
                if !path.is_null() {
                    let path = env.auto_local(JString::from(path));
                    let value: String = env.get_string(&path)?.into();
                    if !value.is_empty() {
                        paths.push(value);
                    }
                }
            }
            Ok(paths)
        })();
        // A provider error must not prevent closing the cursor.
        if result.is_err() {
            let _ = env.exception_clear();
        }
        let close = env.call_method(&cursor, "close", "()V", &[]);
        match result {
            Err(error) => Err(error),
            Ok(paths) => {
                close?;
                Ok(paths)
            }
        }
    })
}
