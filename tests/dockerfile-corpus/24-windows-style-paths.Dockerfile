FROM mcr.microsoft.com/windows/nanoserver:ltsc2022
COPY app.exe C:\\app\\app.exe
CMD ["C:\\app\\app.exe"]
