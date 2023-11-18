# scr
A simple directory tree scanner.
Intended for use in finding large files taking up storage.

Examples:

```
> scanr C:\Users -n 5
Scanning for largest 5 files..
 
    Size(bytes)   created     modified    accessed     path
    167,205,677  2023-11-04  2023-11-04  2023-11-04  C:/Users/mrpsn/AppData/Roaming/Code/CachedExtensionVSIXs/ms-dotnettools.csharp-2.9.20-win32-x64
    166,527,824  2022-07-20  2021-12-28  2022-07-20  C:/Users/mrpsn/AppData/Local/JxBrowser/7.21.2/chrome.dll
    163,654,880  2023-10-06  2023-10-19  2023-11-12  C:/Users/mrpsn/AppData/Local/Programs/signal-desktop/Signal.exe
    163,290,848  2023-10-01  2023-09-27  2023-11-12  C:/Users/mrpsn/AppData/Local/Obsidian/Obsidian.exe
    154,687,920  2023-10-08  2023-11-01  2023-11-12  C:/Users/mrpsn/AppData/Local/Programs/Microsoft VS Code/Code.exe
```


```
> scanr "c:\Program Files"
Scanning for largest 10 files..

    Size(bytes)   created     modified    accessed     path
    738,013,184  2023-06-03  2023-06-17  2023-06-17  c:\Program Files\Docker\Docker\resources\services.iso
    514,351,630  2023-06-15  2023-06-15  2023-11-16  c:\Program Files\Common Files\Adobe\Acrobat\Setup\{AC76BA86-1033-1033-7760-BC15014EA700}\Core.cab
    454,400,000  2023-06-03  2023-06-03  2023-06-03  c:\Program Files\Docker\Docker\resources\wsl\docker-wsl-cli.iso
    408,756,736  2023-10-17  2023-10-17  2023-11-16  c:\Program Files\WSL\system.vhd
    343,799,808  2023-06-03  2023-06-17  2023-06-17  c:\Program Files\Docker\Docker\resources\docker-desktop.iso
    324,608,000  2023-06-15  2023-06-15  2023-07-02  c:\Program Files\Common Files\Adobe\Acrobat\Setup\{AC76BA86-1033-1033-7760-BC15014EA700}\AcroRdrDCx64Upd2300320215.msp
    176,624,592  2023-11-16  2023-06-14  2023-11-18  c:\Program Files\Adobe\Acrobat DC\Acrobat\acrocef_1\libcef.dll
    176,624,592  2023-06-14  2023-06-14  2023-11-16  c:\Program Files\Adobe\Acrobat DC\Acrobat\AcroCEF\libcef.dll
    175,368,192  2023-06-03  2023-06-17  2023-06-17  c:\Program Files\Docker\Docker\resources\wsl\docker-for-wsl.iso
    158,019,504  2023-06-17  2023-04-27  2023-11-18  c:\Program Files\Tabby\Tabby.exe


scanned 144,056 files, 17,993 directories in 1.843 seconds
file loading errors: 0
```