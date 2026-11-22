//! Parameterized localized formatting helpers.

use super::{Locale, locale};

pub(crate) fn play_time(hours: u64, minutes: u64) -> String {
    match locale() {
        Locale::ZhHans => match (hours, minutes) {
            (0, m) => format!("{m} 分钟"),
            (h, 0) => format!("{h} 小时"),
            (h, m) => format!("{h} 小时 {m} 分钟"),
        },
        Locale::Ja => match (hours, minutes) {
            (0, m) => format!("{m} 分"),
            (h, 0) => format!("{h} 時間"),
            (h, m) => format!("{h} 時間 {m} 分"),
        },
        Locale::En => match (hours, minutes) {
            (0, m) => format!("{m} min"),
            (h, 0) => format!("{h} hr"),
            (h, m) => format!("{h} hr {m} min"),
        },
        Locale::Ko => match (hours, minutes) {
            (0, m) => format!("{m}분"),
            (h, 0) => format!("{h}시간"),
            (h, m) => format!("{h}시간 {m}분"),
        },
    }
}

pub(crate) fn statistics_duration(minutes: u64) -> String {
    if minutes < 60 {
        play_time(0, minutes)
    } else {
        play_time(minutes / 60, minutes % 60)
    }
}

pub(crate) fn statistics_month(month: u32) -> String {
    match locale() {
        Locale::ZhHans => format!("{month}月"),
        Locale::Ja => format!("{month}月"),
        Locale::En => match month {
            1 => "Jan",
            2 => "Feb",
            3 => "Mar",
            4 => "Apr",
            5 => "May",
            6 => "Jun",
            7 => "Jul",
            8 => "Aug",
            9 => "Sep",
            10 => "Oct",
            11 => "Nov",
            12 => "Dec",
            _ => "",
        }
        .to_owned(),
        Locale::Ko => format!("{month}월"),
    }
}





















pub(crate) fn sort_tooltip(current: &str, reversed: bool) -> String {
    match locale() {
        Locale::ZhHans => {
            let order = if reversed { " · 倒序" } else { "" };
            format!("排序与更多（当前：{current}{order}）")
        }
        Locale::Ja => {
            let order = if reversed { " · 降順" } else { "" };
            format!("並び替えとその他（現在：{current}{order}）")
        }
        Locale::En => {
            let order = if reversed { " · Descending" } else { "" };
            format!("Sort & More (Current: {current}{order})")
        }
        Locale::Ko => {
            let order = if reversed { " · 내림차순" } else { "" };
            format!("정렬 및 더 보기(현재: {current}{order})")
        }
    }
}



















pub(crate) fn engine_features_summary(unverified: &[&str], unavailable: &[&str]) -> String {
    match locale() {
        Locale::ZhHans => {
            let mut summary = String::new();
            if !unverified.is_empty() {
                summary.push_str(&format!("未验证（需确认）: {}", unverified.join("、")));
            }
            if !unavailable.is_empty() {
                if !summary.is_empty() {
                    summary.push_str("；");
                }
                summary.push_str(&format!("不可用: {}", unavailable.join("、")));
            }
            summary
        }
        Locale::Ja => {
            let mut summary = String::new();
            if !unverified.is_empty() {
                summary.push_str(&format!("未検証（要確認）: {}", unverified.join("、")));
            }
            if !unavailable.is_empty() {
                if !summary.is_empty() {
                    summary.push_str("；");
                }
                summary.push_str(&format!("利用不可: {}", unavailable.join("、")));
            }
            summary
        }
        Locale::En => {
            let mut summary = String::new();
            if !unverified.is_empty() {
                summary.push_str(&format!("Unverified (requires confirmation): {}", unverified.join(", ")));
            }
            if !unavailable.is_empty() {
                if !summary.is_empty() {
                    summary.push_str("; ");
                }
                summary.push_str(&format!("Unavailable: {}", unavailable.join(", ")));
            }
            summary
        }
        Locale::Ko => {
            let mut summary = String::new();
            if !unverified.is_empty() {
                summary.push_str(&format!("미검증(확인 필요): {}", unverified.join(", ")));
            }
            if !unavailable.is_empty() {
                if !summary.is_empty() {
                    summary.push_str("; ");
                }
                summary.push_str(&format!("사용 불가: {}", unavailable.join(", ")));
            }
            summary
        }
    }
}















pub(crate) fn sidebar_title_with_bottle(page: &str, bottle: &str) -> String {
    format!("{page} · {bottle}")
}











































































pub(crate) fn notice_bottle_renamed(name: Option<&str>) -> String {
    match (locale(), name) {
        (Locale::ZhHans, Some(n)) => format!("容器已重命名为 {n}"),
        (Locale::ZhHans, None) => "容器已重命名".to_owned(),
        (Locale::Ja, Some(n)) => format!("コンテナ名を {n} に変更しました"),
        (Locale::Ja, None) => "コンテナ名を変更しました".to_owned(),
        (Locale::En, Some(n)) => format!("Bottle renamed to {n}"),
        (Locale::En, None) => "Bottle renamed".to_owned(),
        (Locale::Ko, Some(n)) => format!("컨테이너 이름을 {n}(으)로 변경했습니다"),
        (Locale::Ko, None) => "컨테이너 이름을 변경했습니다".to_owned(),
    }
}

pub(crate) fn notice_bottle_duplicated(name: Option<&str>) -> String {
    match (locale(), name) {
        (Locale::ZhHans, Some(n)) => format!("已复制容器 {n}"),
        (Locale::ZhHans, None) => "已复制容器".to_owned(),
        (Locale::Ja, Some(n)) => format!("コンテナ {n} を複製しました"),
        (Locale::Ja, None) => "コンテナを複製しました".to_owned(),
        (Locale::En, Some(n)) => format!("Duplicated bottle {n}"),
        (Locale::En, None) => "Bottle duplicated".to_owned(),
        (Locale::Ko, Some(n)) => format!("컨테이너 {n}을(를) 복제했습니다"),
        (Locale::Ko, None) => "컨테이너를 복제했습니다".to_owned(),
    }
}

pub(crate) fn notice_bottle_exported(path: Option<&str>) -> String {
    match (locale(), path) {
        (Locale::ZhHans, Some(p)) => format!("容器已导出到 {p}"),
        (Locale::ZhHans, None) => "容器已导出".to_owned(),
        (Locale::Ja, Some(p)) => format!("コンテナを {p} にエクスポートしました"),
        (Locale::Ja, None) => "コンテナをエクスポートしました".to_owned(),
        (Locale::En, Some(p)) => format!("Bottle exported to {p}"),
        (Locale::En, None) => "Bottle exported".to_owned(),
        (Locale::Ko, Some(p)) => format!("컨테이너를 {p}(으)로 내보냈습니다"),
        (Locale::Ko, None) => "컨테이너를 내보냈습니다".to_owned(),
    }
}







pub(crate) fn notice_gamedb_applied(count: usize, missing: Option<&str>) -> String {
    match (locale(), missing) {
        (Locale::ZhHans, None) => format!("已应用 {count} 项推荐设置。"),
        (Locale::ZhHans, Some(m)) => format!("已应用 {count} 项推荐设置；仍需安装组件：{m}"),
        (Locale::Ja, None) => format!("{count} 件の推奨設定を適用しました。"),
        (Locale::Ja, Some(m)) => format!("{count} 件の推奨設定を適用しました。追加コンポーネントが必要：{m}"),
        (Locale::En, None) => format!("Applied {count} recommended setting(s)."),
        (Locale::En, Some(m)) => format!("Applied {count} recommended setting(s); still need components: {m}"),
        (Locale::Ko, None) => format!("권장 설정 {count}개를 적용했습니다."),
        (Locale::Ko, Some(m)) => format!("권장 설정 {count}개를 적용했습니다. 추가 구성 요소 필요: {m}"),
    }
}

pub(crate) fn notice_sync_done(matched: usize, failed: usize) -> String {
    match locale() {
        Locale::ZhHans => {
            if failed > 0 {
                format!("游戏信息同步完成：匹配 {matched} 个，失败 {failed} 个")
            } else {
                format!("游戏信息同步完成：匹配 {matched} 个")
            }
        }
        Locale::Ja => {
            if failed > 0 {
                format!("メタデータ同期完了：{matched} 件一致、{failed} 件失敗")
            } else {
                format!("メタデータ同期完了：{matched} 件一致")
            }
        }
        Locale::En => {
            if failed > 0 {
                format!("Metadata sync complete: {matched} matched, {failed} failed")
            } else {
                format!("Metadata sync complete: {matched} matched")
            }
        }
        Locale::Ko => {
            if failed > 0 {
                format!("메타데이터 동기화 완료: {matched}개 일치, {failed}개 실패")
            } else {
                format!("메타데이터 동기화 완료: {matched}개 일치")
            }
        }
    }
}

pub(crate) fn notice_wine_installed(version: Option<&str>) -> String {
    match (locale(), version) {
        (Locale::ZhHans, Some(v)) => format!("Wine 运行时 {v} 安装完成"),
        (Locale::ZhHans, None) => "Wine 运行时安装完成".to_owned(),
        (Locale::Ja, Some(v)) => format!("Wine ランタイム {v} のインストールが完了しました"),
        (Locale::Ja, None) => "Wine ランタイムのインストールが完了しました".to_owned(),
        (Locale::En, Some(v)) => format!("Wine runtime {v} installed"),
        (Locale::En, None) => "Wine runtime installed".to_owned(),
        (Locale::Ko, Some(v)) => format!("Wine 런타임 {v} 설치 완료"),
        (Locale::Ko, None) => "Wine 런타임 설치 완료".to_owned(),
    }
}





























































pub(crate) fn overlay_opacity_label(pct: f32) -> String {
    match locale() {
        Locale::ZhHans => format!("{pct:.0}%（0%–100%，0% 全透明）"),
        Locale::Ja => format!("{pct:.0}%（0%–100%、0% で完全透過）"),
        Locale::En => format!("{pct:.0}% (0%–100%, 0% fully transparent)"),
        Locale::Ko => format!("{pct:.0}% (0%–100%, 0% 완전 투명)"),
    }
}





pub(crate) fn capture_and_subtitle_errors(subtitle_error: &str, capture_error: &str) -> String {
    match (locale(), subtitle_error.is_empty(), capture_error.is_empty()) {
        (Locale::ZhHans, false, false) => format!("翻译错误：{subtitle_error}；捕获错误：{capture_error}"),
        (Locale::ZhHans, false, true) => format!("翻译错误：{subtitle_error}"),
        (Locale::ZhHans, true, false) => format!("捕获错误：{capture_error}"),
        (Locale::Ja, false, false) => format!("翻訳エラー：{subtitle_error}；キャプチャエラー：{capture_error}"),
        (Locale::Ja, false, true) => format!("翻訳エラー：{subtitle_error}"),
        (Locale::Ja, true, false) => format!("キャプチャエラー：{capture_error}"),
        (Locale::En, false, false) => format!("Translation error: {subtitle_error}; Capture error: {capture_error}"),
        (Locale::En, false, true) => format!("Translation error: {subtitle_error}"),
        (Locale::En, true, false) => format!("Capture error: {capture_error}"),
        (Locale::Ko, false, false) => format!("번역 오류: {subtitle_error}; 캡처 오류: {capture_error}"),
        (Locale::Ko, false, true) => format!("번역 오류: {subtitle_error}"),
        (Locale::Ko, true, false) => format!("캡처 오류: {capture_error}"),
        (_, true, true) => String::new(),
    }
}
