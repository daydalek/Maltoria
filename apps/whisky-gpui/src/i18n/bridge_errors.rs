//! Localized descriptions for bridge `{code, message}` failures.
//!
//! The bridge reports stable error codes; this module maps them to translated
//! text so the UI can stay localized while the code remains visible for
//! diagnosis. Unknown codes fall back to the raw `{code}: {message}` pair.

use super::tr;

/// Render a bridge failure for display.
///
/// `{code}: {localized}` for known codes; codes whose message carries unique
/// context keep the original message appended; anything else falls back to
/// `{code}: {message}`.
pub(crate) fn describe(code: &str, message: &str) -> String {
    if code.is_empty() {
        return message.to_owned();
    }
    if let Some(inner) = code.strip_prefix("capture_") {
        return match lookup(CAPTURE_TEXT, inner) {
            Some(text) => format!("{code}: {text}"),
            None => fallback(code, message),
        };
    }
    if let Some(text) = lookup(STATIC_TEXT, code) {
        return format!("{code}: {text}");
    }
    if let Some(text) = lookup(DETAIL_TEXT, code) {
        return if message.is_empty() {
            format!("{code}: {text}")
        } else {
            format!("{code}: {text}: {message}")
        };
    }
    fallback(code, message)
}

/// Localized text for a bare error code (capture HUD `lastError`, subtitle
/// `error`, and the `capture_*` hook codes). `None` when the code is unknown.
pub(crate) fn code_text(code: &str) -> Option<&'static str> {
    if let Some(inner) = code.strip_prefix("capture_") {
        return lookup(CAPTURE_TEXT, inner);
    }
    lookup(COMMON_TEXT, code)
        .or_else(|| lookup(STATIC_TEXT, code))
        .or_else(|| lookup(DETAIL_TEXT, code))
        .or_else(|| lookup(CAPTURE_TEXT, code))
}

fn fallback(code: &str, message: &str) -> String {
    if message.is_empty() {
        code.to_owned()
    } else {
        format!("{code}: {message}")
    }
}

fn lookup(
    table: &'static [(&'static str, &'static str, &'static str, &'static str, &'static str)],
    code: &str,
) -> Option<&'static str> {
    table
        .iter()
        .find(|row| row.0 == code)
        .map(|row| tr(row.1, row.2, row.3, row.4))
}

/// Codes whose bridge message is a fixed template; the translation replaces it.
const STATIC_TEXT: &'static [(&'static str, &'static str, &'static str, &'static str, &'static str)] = &[
    ("busy", "另一个操作正在进行，请稍后重试", "別の操作が進行中です。しばらくしてから再試行してください", "Another operation is in progress; try again later", "다른 작업이 진행 중입니다. 잠시 후 다시 시도하세요"),
    ("cancelled", "操作已取消", "操作がキャンセルされました", "The operation was cancelled", "작업이 취소되었습니다"),
    ("candidate_changed", "目录内容已变化，请重新扫描", "ディレクトリが変更されました。再スキャンしてください", "The directory changed; scan again", "디렉터리가 변경되었습니다. 다시 스캔하세요"),
    ("confirmation_required", "需要确认后才能继续", "続行には確認が必要です", "Explicit confirmation is required", "계속하려면 확인이 필요합니다"),
    ("container_busy", "容器正忙，请先停止容器", "コンテナがビジーです。先に停止してください", "The container is busy; stop it first", "컨테이너가 사용 중입니다. 먼저 중지하세요"),
    ("container_unavailable", "容器目录不可用", "コンテナディレクトリが利用できません", "The container directory is unavailable", "컨테이너 디렉터리를 사용할 수 없습니다"),
    ("database_unavailable", "参考数据库读取失败，请恢复应用资源后重试", "参照データベースを読み取れません。アプリリソースを復元して再試行してください", "The reference database could not be read; restore the app resources and retry", "참조 데이터베이스를 읽을 수 없습니다. 앱 리소스를 복원한 후 다시 시도하세요"),
    ("destination_exists", "目标位置已存在同名内容", "保存先に同名の項目がすでに存在します", "The destination already exists", "대상 위치에 이미 항목이 있습니다"),
    ("destination_unavailable", "目标位置不可用", "保存先が利用できません", "The destination is unavailable", "대상 위치를 사용할 수 없습니다"),
    ("display_unavailable", "无法获取显示器信息", "ディスプレイ情報を取得できません", "Display dimensions are unavailable", "디스플레이 정보를 사용할 수 없습니다"),
    ("download_in_progress", "请先取消正在进行的下载", "進行中のダウンロードを先にキャンセルしてください", "Cancel the active download first", "진행 중인 다운로드를 먼저 취소하세요"),
    ("engine_busy", "Wine 引擎正忙，请稍后重试", "Wine エンジンがビジーです。しばらくしてから再試行してください", "The Wine engine registry is busy", "Wine 엔진 레지스트리가 사용 중입니다"),
    ("engine_in_use", "Wine 引擎正在使用中", "Wine エンジンが使用中です", "The Wine engine is in use", "Wine 엔진이 사용 중입니다"),
    ("engine_unavailable", "Wine 引擎不可用", "Wine エンジンが利用できません", "The Wine engine is unavailable", "Wine 엔진을 사용할 수 없습니다"),
    ("host_timeout", "文件选择超时；请重新打开选择器", "ファイル選択がタイムアウトしました。ピッカーを開き直してください", "File selection timed out; open the picker again", "파일 선택 시간이 초과되었습니다. 선택기를 다시 여세요"),
    ("identity_limit", "已达到容器身份数量上限", "コンテナIDの上限に達しました", "The container identity limit was reached", "컨테이너 ID 한도에 도달했습니다"),
    ("journal_too_large", "容器操作日志超出大小限制", "コンテナジャーナルがサイズ上限を超えています", "The container journal exceeds the size limit", "컨테이너 저널이 크기 제한을 초과했습니다"),
    ("launcher_unavailable", "启动器不可用", "ランチャーが利用できません", "The launcher is unavailable", "런처를 사용할 수 없습니다"),
    ("managed_immutable", "该项由外部管理，不可修改", "外部管理の項目は変更できません", "This item is managed externally and cannot be modified", "외부에서 관리되는 항목은 변경할 수 없습니다"),
    ("offline", "离线模式下该操作不可用", "オフラインモードではこの操作は利用できません", "This operation is unavailable in offline mode", "오프라인 모드에서는 이 작업을 사용할 수 없습니다"),
    ("output_too_large", "结果超出大小限制", "結果がサイズ上限を超えています", "The result exceeds the size limit", "결과가 크기 제한을 초과했습니다"),
    ("overlay_persistence_failed", "无法保存字幕叠层设置", "字幕オーバーレイ設定を保存できません", "Could not save subtitle overlay settings", "자막 오버레이 설정을 저장할 수 없습니다"),
    ("prefix_boot_failed", "Wine 前缀初始化失败", "Wine プレフィックスの初期化に失敗しました", "Wine did not create a usable prefix", "Wine 프리픽스 초기화에 실패했습니다"),
    ("prefix_invalid", "Wine 前缀不完整，请重建或修复容器", "Wine プレフィックスが不完全です。ボトルを再作成または修復してください", "The Wine prefix is incomplete; recreate or repair the bottle", "Wine 프리픽스가 손상되었습니다. 병을 다시 만들거나 복구하세요"),
    ("provider_limit", "最多只能保存 64 个服务商", "プロバイダーは最大64件まで保存できます", "At most 64 providers can be saved", "프로바이더는 최대 64개까지 저장할 수 있습니다"),
    ("provider_not_found", "找不到该服务商配置", "プロバイダー設定が見つかりません", "The provider profile was not found", "프로바이더 프로필을 찾을 수 없습니다"),
    ("read_only_workspace", "需要先保存容器注册表", "永続化されたレジストリが必要です", "This change requires a persisted registry", "저장된 레지스트리가 필요합니다"),
    ("release_unavailable", "没有可用的已发布运行时", "公開済みランタイムがありません", "No published runtime release is available", "사용 가능한 릴리스 런타임이 없습니다"),
    ("remote_providers_disabled", "请先在翻译设置中启用远程服务商", "翻訳設定でリモートプロバイダーを有効にしてください", "Enable remote providers in translation settings first", "번역 설정에서 원격 프로바이더를 먼저 활성화하세요"),
    ("runtime_missing", "缺少运行时资源", "ランタイムリソースがありません", "A runtime resource is missing", "런타임 리소스가 없습니다"),
    ("runtime_unavailable", "运行时不可用", "ランタイムが利用できません", "The runtime is unavailable", "런타임을 사용할 수 없습니다"),
    ("session_closed", "桥接会话已关闭", "ブリッジセッションが閉じられました", "The bridge session is closed", "브리지 세션이 닫혔습니다"),
    ("translation_operation_failed", "翻译操作失败", "翻訳操作に失敗しました", "The translation operation failed", "번역 작업에 실패했습니다"),
    ("translation_storage_unavailable", "无法读取或保存翻译设置", "翻訳設定を読み書きできません", "Translation settings could not be read or saved", "번역 설정을 읽거나 저장할 수 없습니다"),
    ("unknown_container", "容器不在当前工作区内", "コンテナがこのワークスペース外にあります", "The container is outside this workspace", "컨테이너가 이 작업 공간 밖에 있습니다"),
    ("unknown_job", "任务不存在或已过期", "ジョブが存在しないか期限切れです", "The operation is no longer retained", "작업이 더 이상 존재하지 않습니다"),
    ("unmanaged_runtime", "检测到非托管的运行时目录", "管理外のランタイムディレクトリが検出されました", "An unmanaged runtime directory was detected", "관리되지 않는 런타임 디렉터리가 감지되었습니다"),
    ("unsupported_platform", "当前平台不支持此操作", "このプラットフォームではサポートされていません", "This operation is not supported on this platform", "이 플랫폼에서는 지원되지 않습니다"),
    ("version_conflict", "版本冲突，请更新后重试", "バージョンが競合しています。更新してから再試行してください", "A version conflict occurred", "버전 충돌이 발생했습니다. 업데이트 후 다시 시도하세요"),
];

/// Codes whose bridge message carries unique context; the original message is
/// kept after the translated category.
const DETAIL_TEXT: &'static [(&'static str, &'static str, &'static str, &'static str, &'static str)] = &[
    ("capability_unavailable", "当前功能不可用", "この機能は利用できません", "This capability is unavailable", "이 기능을 사용할 수 없습니다"),
    ("checksum_failed", "文件校验和计算失败", "チェックサムの計算に失敗しました", "Checksum computation failed", "체크섬 계산에 실패했습니다"),
    ("checksum_mismatch", "文件校验和不匹配", "チェックサムが一致しません", "Checksum mismatch", "체크섬이 일치하지 않습니다"),
    ("desktop_update_recovery", "桌面项更新需要恢复处理", "デスクトップ項目の更新は復旧が必要です", "The desktop entry update requires recovery", "데스크톱 항목 업데이트에 복구가 필요합니다"),
    ("download_failed", "下载失败", "ダウンロードに失敗しました", "Download failed", "다운로드에 실패했습니다"),
    ("download_http_error", "下载返回了 HTTP 错误", "ダウンロードがHTTPエラーを返しました", "The download returned an HTTP error", "다운로드가 HTTP 오류를 반환했습니다"),
    ("download_io_error", "下载文件写入失败", "ダウンロードファイルの書き込みに失敗しました", "Could not write the downloaded file", "다운로드한 파일을 쓸 수 없습니다"),
    ("engine_rejected", "Wine 引擎被拒绝", "Wine エンジンが拒否されました", "The Wine engine was rejected", "Wine 엔진이 거부되었습니다"),
    ("entry_move_failed", "移动条目失败", "エントリの移動に失敗しました", "Could not move the entry", "항목을 이동할 수 없습니다"),
    ("extract_failed", "解压失败", "展開に失敗しました", "Extraction failed", "압축 해제에 실패했습니다"),
    ("file_missing", "所选文件不可用", "選択したファイルが利用できません", "The selected file is unavailable", "선택한 파일을 사용할 수 없습니다"),
    ("host_failed", "宿主操作失败", "ホスト操作に失敗しました", "The host operation failed", "호스트 작업에 실패했습니다"),
    ("identity_store_invalid", "容器身份记录无效", "コンテナIDレコードが無効です", "Invalid container identity records", "컨테이너 ID 레코드가 유효하지 않습니다"),
    ("import_failed", "导入失败", "インポートに失敗しました", "Import failed", "가져오기에 실패했습니다"),
    ("install_failed", "安装失败", "インストールに失敗しました", "Installation failed", "설치에 실패했습니다"),
    ("invalid_container", "容器数据无效", "コンテナデータが無効です", "Invalid container data", "컨테이너 데이터가 유효하지 않습니다"),
    ("invalid_manifest", "清单数据无效", "マニフェストが無効です", "Invalid manifest", "매니페스트가 유효하지 않습니다"),
    ("invalid_model_url", "模型下载地址无效", "モデルのダウンロードURLが無効です", "The model download URL is invalid", "모델 다운로드 URL이 유효하지 않습니다"),
    ("invalid_params", "请求参数无效", "リクエストパラメータが無効です", "Invalid parameters", "요청 매개변수가 유효하지 않습니다"),
    ("invalid_request", "请求无效", "リクエストが無効です", "Invalid request", "유효하지 않은 요청입니다"),
    ("invalid_runtime_archive", "运行时归档无效", "ランタイムアーカイブが無効です", "Invalid runtime archive", "런타임 아카이브가 유효하지 않습니다"),
    ("invalid_runtime_url", "运行时下载地址无效", "ランタイムのダウンロードURLが無効です", "Invalid runtime download URL", "런타임 다운로드 URL이 유효하지 않습니다"),
    ("invalid_settings", "设置数据无效", "設定データが無効です", "Invalid settings data", "설정 데이터가 유효하지 않습니다"),
    ("launch_failed", "启动失败", "起動に失敗しました", "Launch failed", "실행에 실패했습니다"),
    ("method_unavailable", "该方法不可用", "このメソッドは利用できません", "The method is unavailable", "메서드를 사용할 수 없습니다"),
    ("model_not_found", "找不到指定的模型", "モデルが見つかりません", "The model was not found", "모델을 찾을 수 없습니다"),
    ("operation_failed", "操作失败", "操作に失敗しました", "The operation failed", "작업에 실패했습니다"),
    ("process_query_failed", "进程查询失败", "プロセス照会に失敗しました", "Process query failed", "프로세스 조회에 실패했습니다"),
    ("recovery_required", "需要恢复中断的操作", "中断された操作の復旧が必要です", "Recovery is required for an interrupted operation", "중단된 작업의 복구가 필요합니다"),
    ("registry_write_failed", "注册表写入失败", "レジストリへの書き込みに失敗しました", "Could not write to the registry", "레지스트리에 쓸 수 없습니다"),
    ("route_store_invalid", "路由存储数据无效", "ルートストアが無効です", "Invalid route store", "경로 저장소가 유효하지 않습니다"),
    ("runtime_busy", "运行时正忙", "ランタイムがビジーです", "The runtime is busy", "런타임이 사용 중입니다"),
    ("shortcut_unavailable", "快捷方式不可用", "ショートカットが利用できません", "The shortcut is unavailable", "바로 가기를 사용할 수 없습니다"),
    ("transport_failed", "传输失败", "転送に失敗しました", "Transport failed", "전송에 실패했습니다"),
    ("translation_error", "翻译服务错误", "翻訳サービスエラー", "Translation error", "번역 오류"),
    ("truncated_frame", "数据帧不完整", "フレームが途中で切れています", "Truncated frame", "프레임이 잘렸습니다"),
    ("unknown_engine", "未知的 Wine 引擎", "不明なWineエンジンです", "Unknown Wine engine", "알 수 없는 Wine 엔진입니다"),
    ("unknown_entry", "未知的条目", "不明なエントリです", "Unknown entry", "알 수 없는 항목입니다"),
];

/// Bare codes the capture HUD and subtitle overlay report without a
/// `{code, message}` pair, plus hook error codes used by `capture_*` failures.
const COMMON_TEXT: &'static [(&'static str, &'static str, &'static str, &'static str, &'static str)] = &[
    ("timeout", "翻译超时", "翻訳がタイムアウトしました", "Translation timed out", "번역 시간 초과"),
    ("capture_error", "捕获错误", "キャプチャエラー", "Capture error", "캡처 오류"),
    ("translation_failed", "翻译失败", "翻訳に失敗しました", "Translation failed", "번역에 실패했습니다"),
    ("thread_limit", "文本线程数量超限", "テキストスレッド数が上限を超えています", "Too many text threads", "텍스트 스레드 수가 한도를 초과했습니다"),
    ("Translation cancelled", "翻译已取消", "翻訳がキャンセルされました", "Translation cancelled", "번역이 취소되었습니다"),
    ("Translation request failed", "翻译请求失败", "翻訳リクエストに失敗しました", "Translation request failed", "번역 요청에 실패했습니다"),
];

/// Inner codes of `capture_*` hook failures (`hookFailure` in the bridge).
const CAPTURE_TEXT: &'static [(&'static str, &'static str, &'static str, &'static str, &'static str)] = &[
    ("accept_failed", "无法接入文本钩子连接", "テキストフック接続を受け付けられません", "Could not accept the text-hook connection", "텍스트 후크 연결을 수락할 수 없습니다"),
    ("already_attached", "已附加到目标进程", "すでに対象プロセスにアタッチされています", "Already attached to the target process", "이미 대상 프로세스에 연결되어 있습니다"),
    ("authentication_failed", "文本钩子认证失败", "テキストフック認証に失敗しました", "Text-hook authentication failed", "텍스트 후크 인증에 실패했습니다"),
    ("closed", "文本钩子连接已关闭", "テキストフック接続が閉じられました", "The text-hook connection is closed", "텍스트 후크 연결이 닫혔습니다"),
    ("connect_timeout", "文本钩子连接超时", "テキストフック接続がタイムアウトしました", "The text-hook connection timed out", "텍스트 후크 연결 시간이 초과되었습니다"),
    ("disconnected", "文本钩子连接已断开", "テキストフック接続が切断されました", "The text-hook connection was disconnected", "텍스트 후크 연결이 끊어졌습니다"),
    ("events_dropped", "事件队列溢出，已丢弃部分文本事件", "イベントキューが溢れ、一部のテキストイベントが破棄されました", "The event queue overflowed; some text events were dropped", "이벤트 큐가 넘쳐 일부 텍스트 이벤트가 삭제되었습니다"),
    ("frame_too_large", "文本帧超出大小限制", "テキストフレームがサイズ上限を超えています", "The text frame exceeds the size limit", "텍스트 프레임이 크기 제한을 초과했습니다"),
    ("history_mismatch", "捕获历史记录不匹配", "キャプチャ履歴が一致しません", "The capture history does not match", "캡처 기록이 일치하지 않습니다"),
    ("history_too_large", "捕获历史记录超出大小限制", "キャプチャ履歴がサイズ上限を超えています", "The capture history exceeds the size limit", "캡처 기록이 크기 제한을 초과했습니다"),
    ("invalid_architecture", "目标进程架构不受支持", "対象プロセスのアーキテクチャは未対応です", "The target process architecture is unsupported", "대상 프로세스 아키텍처가 지원되지 않습니다"),
    ("invalid_frame", "无效的数据帧", "無効なフレームです", "Invalid frame", "유효하지 않은 프레임입니다"),
    ("invalid_game_id", "无效的游戏标识", "無効なゲームIDです", "Invalid game ID", "유효하지 않은 게임 ID입니다"),
    ("invalid_hook_code", "无效的钩子代码", "無効なフックコードです", "Invalid hook code", "유효하지 않은 후크 코드입니다"),
    ("invalid_params", "无效的捕获参数", "無効なキャプチャパラメータです", "Invalid capture parameters", "유효하지 않은 캡처 매개변수입니다"),
    ("invalid_process_list", "无效的进程列表", "無効なプロセス一覧です", "Invalid process list", "유효하지 않은 프로세스 목록입니다"),
    ("invalid_provider", "无效的翻译服务商", "無効な翻訳プロバイダーです", "Invalid translation provider", "유효하지 않은 번역 프로바이더입니다"),
    ("invalid_target", "无效的捕获目标", "無効なキャプチャ対象です", "Invalid capture target", "유효하지 않은 캡처 대상입니다"),
    ("invalid_threads", "无效的线程选择", "無効なスレッド選択です", "Invalid thread selection", "유효하지 않은 스레드 선택입니다"),
    ("not_attached", "尚未附加到目标进程", "対象プロセスにアタッチされていません", "Not attached to the target process", "대상 프로세스에 연결되어 있지 않습니다"),
    ("not_started", "捕获尚未开始", "キャプチャが開始されていません", "Capture has not started", "캡처가 시작되지 않았습니다"),
    ("offline", "捕获通道离线", "キャプチャチャネルがオフラインです", "The capture channel is offline", "캡처 채널이 오프라인입니다"),
    ("payload_unavailable", "捕获数据不可用", "キャプチャデータが利用できません", "The capture payload is unavailable", "캡처 페이로드를 사용할 수 없습니다"),
    ("preset_too_large", "捕获预设超出大小限制", "キャプチャプリセットがサイズ上限を超えています", "The capture preset exceeds the size limit", "캡처 프리셋이 크기 제한을 초과했습니다"),
    ("receive_failed", "接收捕获数据失败", "キャプチャデータの受信に失敗しました", "Could not receive capture data", "캡처 데이터를 수신하지 못했습니다"),
    ("request_timeout", "捕获请求超时", "キャプチャリクエストがタイムアウトしました", "The capture request timed out", "캡처 요청 시간이 초과되었습니다"),
    ("runtime_unavailable", "捕获运行时不可用", "キャプチャランタイムが利用できません", "The capture runtime is unavailable", "캡처 런타임을 사용할 수 없습니다"),
    ("send_failed", "发送捕获数据失败", "キャプチャデータの送信に失敗しました", "Could not send capture data", "캡처 데이터를 보내지 못했습니다"),
    ("socket_unavailable", "捕获套接字不可用", "キャプチャソケットが利用できません", "The capture socket is unavailable", "캡처 소켓을 사용할 수 없습니다"),
    ("test_timeout", "捕获超时", "キャプチャがタイムアウトしました", "Capture timed out", "캡처 시간이 초과되었습니다"),
    ("truncated_frame", "文本帧被截断", "テキストフレームが途中で切れています", "The text frame was truncated", "텍스트 프레임이 잘렸습니다"),
    ("unexpected_frame", "意外的数据帧", "予期しないフレームです", "Unexpected frame", "예기치 않은 프레임입니다"),
    ("unexpected_response", "意外的响应", "予期しない応答です", "Unexpected response", "예기치 않은 응답입니다"),
    ("unknown_entry", "未知的捕获条目", "不明なキャプチャエントリです", "Unknown capture entry", "알 수 없는 캡처 항목입니다"),
    ("wrong_session", "捕获会话不匹配", "キャプチャセッションが一致しません", "The capture session does not match", "캡처 세션이 일치하지 않습니다"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{Locale, LocaleGuard};

    #[test]
    fn describes_known_codes_in_every_locale() {
        for (locale, expected) in [
            (Locale::ZhHans, "translation_storage_unavailable: 无法读取或保存翻译设置"),
            (Locale::Ja, "translation_storage_unavailable: 翻訳設定を読み書きできません"),
            (Locale::En, "translation_storage_unavailable: Translation settings could not be read or saved"),
            (Locale::Ko, "translation_storage_unavailable: 번역 설정을 읽거나 저장할 수 없습니다"),
        ] {
            let _guard = LocaleGuard::new(locale);
            assert_eq!(
                describe("translation_storage_unavailable", "Saved translation settings could not be read"),
                expected
            );
        }
    }

    #[test]
    fn detail_codes_keep_the_original_message() {
        let _guard = LocaleGuard::new(Locale::ZhHans);
        assert_eq!(
            describe("invalid_params", "Expected boolean: useChinaMirror"),
            "invalid_params: 请求参数无效: Expected boolean: useChinaMirror"
        );
        assert_eq!(describe("invalid_params", ""), "invalid_params: 请求参数无效");
    }

    #[test]
    fn unknown_codes_and_empty_code_fall_back() {
        let _guard = LocaleGuard::new(Locale::ZhHans);
        assert_eq!(describe("brand_new_code", "raw message"), "brand_new_code: raw message");
        assert_eq!(describe("brand_new_code", ""), "brand_new_code");
        assert_eq!(describe("", "only message"), "only message");
    }

    #[test]
    fn capture_codes_resolve_through_prefix_and_bare_lookup() {
        let _guard = LocaleGuard::new(Locale::ZhHans);
        assert_eq!(
            describe("capture_disconnected", "Text capture: disconnected"),
            "capture_disconnected: 文本钩子连接已断开"
        );
        assert_eq!(code_text("disconnected"), Some("文本钩子连接已断开"));
        assert_eq!(code_text("timeout"), Some("翻译超时"));
        assert_eq!(code_text("provider_not_found"), Some("找不到该服务商配置"));
        assert_eq!(code_text("nonexistent_code"), None);
    }
}
