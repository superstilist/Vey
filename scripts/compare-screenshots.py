from pathlib import Path
from PIL import Image,ImageOps,ImageDraw
root=Path(__file__).resolve().parent.parent
pairs=[('01-home-default.png','01-home.png','Home'),('04-library.png','04-library.png','Library'),('08-settings.png','08-settings.png','Settings'),('10-nowplaying-lyrics.png','10-nowplaying.png','Now Playing')]
out=Image.new('RGB',(1440,len(pairs)*480),(8,8,12));draw=ImageDraw.Draw(out)
for i,(original,native,label) in enumerate(pairs):
 y=i*480
 draw.text((12,y+7),f'Original - {label}',fill=(245,243,248));draw.text((732,y+7),f'Rust / Slint - {label}',fill=(249,115,22))
 for x,path in [(0,root/'docs/reference'/original),(720,root/'docs/screenshots'/native)]:
  with Image.open(path) as image:out.paste(ImageOps.contain(image.convert('RGB'),(720,450)),(x,y+30))
out.save(root/'docs/screenshots/comparison.png')
print(root/'docs/screenshots/comparison.png')
