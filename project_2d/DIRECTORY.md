# project_2d/ 디렉토리 구조

```
project_2d/
├── Cargo.toml                          # 바이너리 (grid_server) + 라이브러리 (project_2d) 패키지
├── server.toml                         # Grid 서버 설정 파일 (TOML)
├── src/
│   ├── main.rs                         # Grid 서버 진입점 (tokio + tick 스레드, WebSocket, AOI)
│   ├── lib.rs                          # 라이브러리 루트 (pub mod components)
│   ├── components.rs                   # Grid 전용 ECS 컴포넌트 (Name)
│   ├── config.rs                       # 서버 설정 — TOML 파싱, CLI 오버라이드, 기본값
│   └── shutdown.rs                     # 안전 종료 — watch 채널 기반 ShutdownTx/ShutdownRx
├── web_client/                         # 웹 클라이언트 (TypeScript + Vite + PixiJS)
│   ├── package.json                    # 의존성: pixi.js v8, vite v6, typescript v5
│   ├── package-lock.json
│   ├── tsconfig.json                   # 엄격 모드, ES2020, bundler moduleResolution
│   ├── vite.config.ts                  # 개발 프록시 /ws → :4001, 빌드 출력 → ../web_dist/
│   ├── index.html                      # 로그인 오버레이 + canvas 컨테이너
│   └── src/
│       ├── main.ts                     # 진입점 — 모듈 조립, 생명주기 관리
│       ├── protocol.ts                 # 서버 프로토콜 TypeScript 타입 정의
│       ├── state.ts                    # 엔티티 상태 Map + 델타 적용 로직
│       ├── ws.ts                       # WebSocket 연결 관리 (connect/send/close)
│       ├── input.ts                    # WASD 키보드 → Move 메시지 (100ms 쓰로틀)
│       └── renderer.ts                # PixiJS: 그리드 배경, 엔티티 원형, 이름 라벨, 카메라 추적
├── web_dist/                           # 빌드된 웹 클라이언트 산출물 (vite build 결과, 자동 생성)
│   └── ...
└── tests/                              # 통합 테스트 (4개)
    ├── grid_space_test.rs              # GridSpace 배치/이동/제거/반경/범위 검증
    ├── grid_tick_integration.rs        # Grid TickLoop 스텝/명령어/다중 엔티티
    ├── grid_scripting_test.rs          # Grid Lua API (위치/이동/반경/설정)
    └── ws_grid_integration.rs          # WebSocket 종단간 테스트 (접속/이동/AOI/해제)
```
