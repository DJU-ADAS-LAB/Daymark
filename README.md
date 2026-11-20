# Daymark · Rust 체크리스트 개발

바탕화면에 띄우는 달력 스티커형 체크리스트 프로젝트입니다.

## 폴더

- `frontend/` — 동작하는 HTML, CSS, JavaScript 화면
- `rust-practice/src-tauri/` — Windows 위젯과 설치 프로그램
- `docs/mockup.html` — 디자인 목업
- `docs/flowchart.md` — 사용 흐름도와 구현 경계

## Windows에서 실행과 설치 파일 빌드

프로젝트 최상위 폴더의 PowerShell에서 실행합니다.

```powershell
cd rust-practice
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
cargo tauri dev
```

같은 폴더에서 아래 명령으로 Windows 설치 파일을 다시 빌드할 수 있습니다.

```powershell
cargo tauri build --bundles nsis -- --locked
```

설치 파일은 `rust-practice/src-tauri/target/release/bundle/nsis/`에 생성됩니다. 설치하면 현재 사용자 계정에 Daymark가 등록되어 Windows 로그인 시 실행됩니다. Windows의 시작프로그램 설정에서 끌 수 있고, Daymark를 제거하면 등록도 해제됩니다.

할 일, 메모, 창 배치는 `%LOCALAPPDATA%\io.daymark.desktop`에 저장됩니다. 이전 데스크톱 버전이 `%LOCALAPPDATA%\com.tauri.dev`에 저장한 파일은 첫 실행 시 복사됩니다.

공개 배포용으로 서명할 때는 코드 서명 인증서를 Windows의 현재 사용자 개인 인증서 저장소에 준비한 뒤 `rust-practice`에서 실행합니다. `-TimestampUrl`에는 인증서 발급사가 안내하는 RFC 3161 주소를 지정합니다.

```powershell
.\build-signed.ps1 -TimestampUrl '<인증서 발급사의 RFC 3161 주소>'
```

인증서가 여러 개인 경우 `-CertificateThumbprint`로 하나를 지정합니다. 스크립트는 앱과 설치 파일의 게시자 서명 및 타임스탬프를 확인합니다.
