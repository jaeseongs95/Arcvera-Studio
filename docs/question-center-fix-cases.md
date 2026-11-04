# 질문함 회귀 사례 (구현 전 작성)

범위: `createQuestionCenter` 공개 UI와 native 명령 호출. 기존 Playwright fixture/runner를 사용한다. native observer 및 실제 OS 브라우저 실행은 별도 backend/link 사례에서 검증하며 이 UI mock으로 보장하지 않는다.

| ID | 입력 및 전제 | 절차 | 기대 결과 / 판정 기준 |
| --- | --- | --- | --- |
| QC-L1 | 질문 제목·본문·선택지·설명에 HTTP(S) URL 포함, native 호출 기록 가능 | 질문함을 열고 각 링크 클릭 | 모든 URL은 텍스트를 보존하는 링크로 표시, 클릭당 `open_external_url` 1회. 질문 응답/선택 변경 없음 |
| QC-L2 | native opener가 실패 | 링크 클릭 | 질문 원문·초안 보존, 오류를 안전한 안내로 표시. 내부 경로·오류 문자열 노출 없음, 자동 재시도 없음 |
| QC-A1 | `source:async` 질문, 안정 requestId/questionId, 자유 입력 및 선택지 | 초안 작성→목록 갱신→명시 전송 | 초안 유지, 기존 응답 명령에 정확한 세션/세대/질문 ID와 답변 전달, 해결 관측 전 항목 유지 |
| QC-A2 | `complete:false`, 알려진 항목 없음 | 최초 조회·언어 전환·재조회 | 확정 0건/미응답 없음으로 표시하지 않음. 이후 complete:true 빈 목록만 확정 0건 |
| QC-A3 | `complete:false`, 알려진 항목 1개 | 최초 조회·complete:true로 재조회 | 확인된 질문은 유지·응답 가능. 일부 상태 미확인 안내 후 정상 조회 때 안내 해제 |
| QC-R1 | 기존 sync 질문/승인, 연결 끊김, 전송 불명, 숨김·해제 fixture | 기존 question-center.spec.ts 전체 실행 | 모든 기존 요구 보존. 거짓 해결·자동 재전송·중복 poll 없음 |

실행 결과는 테스트의 원시 Playwright 결과와 최종 검증 기록에 남긴다. 이 문서는 실행 성공을 주장하지 않는다.
