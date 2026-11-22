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

pub(crate) fn statistics_day_tooltip(date: impl std::fmt::Display, count: usize, dur: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("{date} · {count} 款游戏 · {dur}"),
        Locale::Ja => format!("{date} · {count} 本のゲーム · {dur}"),
        Locale::En => format!(
            "{date} · {count} {} · {dur}",
            if count == 1 { "game" } else { "games" }
        ),
        Locale::Ko => format!("{date} · 게임 {count}개 · {dur}"),
    }
}

pub(crate) fn statistics_day_summary(count: usize, dur: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("{count} 款游戏 · {dur}"),
        Locale::Ja => format!("{count} 本のゲーム · {dur}"),
        Locale::En => format!(
            "{count} {} · {dur}",
            if count == 1 { "game" } else { "games" }
        ),
        Locale::Ko => format!("게임 {count}개 · {dur}"),
    }
}

pub(crate) fn statistics_active_days(days: usize) -> String {
    match locale() {
        Locale::ZhHans => format!("{days} 天"),
        Locale::Ja => format!("{days} 日"),
        Locale::En => format!("{days} {}", if days == 1 { "day" } else { "days" }),
        Locale::Ko => format!("{days}일"),
    }
}

pub(crate) fn statistics_progress_aria(title: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("{title} 当日时长占比"),
        Locale::Ja => format!("{title} の当日プレイ時間割合"),
        Locale::En => format!("{title} share of daily playtime"),
        Locale::Ko => format!("{title} 당일 플레이 시간 비율"),
    }
}

pub(crate) fn relative_minutes_ago(m: u64) -> String {
    match locale() {
        Locale::ZhHans => format!("{m} 分钟前"),
        Locale::Ja => format!("{m} 分前"),
        Locale::En => format!("{m}m ago"),
        Locale::Ko => format!("{m}분 전"),
    }
}

pub(crate) fn relative_hours_ago(h: u64) -> String {
    match locale() {
        Locale::ZhHans => format!("{h} 小时前"),
        Locale::Ja => format!("{h} 時間前"),
        Locale::En => format!("{h}h ago"),
        Locale::Ko => format!("{h}시간 전"),
    }
}

pub(crate) fn relative_days_ago(d: u64) -> String {
    match locale() {
        Locale::ZhHans => format!("{d} 天前"),
        Locale::Ja => format!("{d} 日前"),
        Locale::En => format!("{d}d ago"),
        Locale::Ko => format!("{d}일 전"),
    }
}

pub(crate) fn appearance_toggled(label: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("已切换外观：{label}"),
        Locale::Ja => format!("外観を切り替えました：{label}"),
        Locale::En => format!("Appearance: {label}"),
        Locale::Ko => format!("화면 모드 변경됨: {label}"),
    }
}

pub(crate) fn appearance_tooltip(label: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("切换外观（当前：{label}）"),
        Locale::Ja => format!("外観を切り替え（現在：{label}）"),
        Locale::En => format!("Switch Appearance (Current: {label})"),
        Locale::Ko => format!("화면 모드 전환(현재: {label})"),
    }
}

pub(crate) fn language_changed(label: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("界面语言已切换：{label}"),
        Locale::Ja => format!("表示言語を切り替えました：{label}"),
        Locale::En => format!("Interface language: {label}"),
        Locale::Ko => format!("인터페이스 언어 변경됨: {label}"),
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

pub(crate) fn sort_by(label: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("按{label}排序"),
        Locale::Ja => format!("{label}順で並び替え"),
        Locale::En => format!("Sort by {label}"),
        Locale::Ko => format!("{label}순으로 정렬"),
    }
}

pub(crate) fn show_hidden_games(count: usize) -> String {
    match locale() {
        Locale::ZhHans => {
            if count == 0 {
                "显示已隐藏的游戏".to_owned()
            } else {
                format!("显示已隐藏的游戏（{count}）")
            }
        }
        Locale::Ja => {
            if count == 0 {
                "非表示のゲームを表示".to_owned()
            } else {
                format!("非表示のゲームを表示（{count}）")
            }
        }
        Locale::En => {
            if count == 0 {
                "Show Hidden Games".to_owned()
            } else {
                format!("Show Hidden Games ({count})")
            }
        }
        Locale::Ko => {
            if count == 0 {
                "숨겨진 게임 표시".to_owned()
            } else {
                format!("숨겨진 게임 표시 ({count})")
            }
        }
    }
}

pub(crate) fn show_hidden_short(count: usize) -> String {
    match locale() {
        Locale::ZhHans => format!("显示已隐藏（{count}）"),
        Locale::Ja => format!("非表示を表示（{count}）"),
        Locale::En => format!("Show Hidden ({count})"),
        Locale::Ko => format!("숨김 표시 ({count})"),
    }
}

pub(crate) fn filter_same_studio(studio: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("查看同厂商作品（{studio}）"),
        Locale::Ja => format!("同じブランドの作品を表示（{studio}）"),
        Locale::En => format!("More from {studio}"),
        Locale::Ko => format!("같은 제작사 작품 보기 ({studio})"),
    }
}

pub(crate) fn studio_count(count: usize) -> String {
    match locale() {
        Locale::ZhHans => format!("{count} 款"),
        Locale::Ja => format!("{count} 本"),
        Locale::En => format!("{count} {}", if count == 1 { "title" } else { "titles" }),
        Locale::Ko => format!("{count}개"),
    }
}

pub(crate) fn games_count(count: usize) -> String {
    match locale() {
        Locale::ZhHans => format!("{count} 款游戏"),
        Locale::Ja => format!("{count} 本のゲーム"),
        Locale::En => format!("{count} {}", if count == 1 { "game" } else { "games" }),
        Locale::Ko => format!("게임 {count}개"),
    }
}

pub(crate) fn cover_batch_cancel(done: usize, total: usize) -> String {
    match locale() {
        Locale::ZhHans => format!("取消（{done}/{total}）"),
        Locale::Ja => format!("キャンセル（{done}/{total}）"),
        Locale::En => format!("Cancel ({done}/{total})"),
        Locale::Ko => format!("취소 ({done}/{total})"),
    }
}

pub(crate) fn cover_batch_done(succeeded: usize, failed: usize) -> String {
    match locale() {
        Locale::ZhHans => format!("封面超分完成：成功 {succeeded} 张，未生成 {failed} 张"),
        Locale::Ja => format!("カバー超解像が完了しました：成功 {succeeded} 件、未生成 {failed} 件"),
        Locale::En => format!("Cover upscaling finished: {succeeded} succeeded, {failed} skipped"),
        Locale::Ko => format!("커버 업스케일링 완료: 성공 {succeeded}개, 건너뜀 {failed}개"),
    }
}

pub(crate) fn engine_registered(version: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("已注册 · {version}"),
        Locale::Ja => format!("登録済み · {version}"),
        Locale::En => format!("Registered · {version}"),
        Locale::Ko => format!("등록됨 · {version}"),
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

pub(crate) fn engine_registry_read_failed(error: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("引擎注册表读取失败：{error}"),
        Locale::Ja => format!("エンジン登録情報の読み込みに失敗しました：{error}"),
        Locale::En => format!("Failed to read engine registry: {error}"),
        Locale::Ko => format!("엔진 레지스트리 읽기 실패: {error}"),
    }
}

pub(crate) fn runtime_installed_version(version: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("已安装 · {version}"),
        Locale::Ja => format!("インストール済み · {version}"),
        Locale::En => format!("Installed · {version}"),
        Locale::Ko => format!("설치됨 · {version}"),
    }
}

pub(crate) fn empty_search_detail(query: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("未找到与「{query}」匹配的游戏、开发商、年份或标签，可尝试更换关键词。"),
        Locale::Ja => format!("「{query}」に一致するゲーム、開発元、発売年、タグが見つかりませんでした。別のキーワードをお試しください。"),
        Locale::En => format!("No games, developers, years, or tags matched \"{query}\". Try a different keyword."),
        Locale::Ko => format!("'{query}'과(와) 일치하는 게임, 개발사, 연도 또는 태그를 찾지 못했습니다. 다른 키워드를 입력해 보세요."),
    }
}

pub(crate) fn empty_studio_and_tag_detail(studio: &str, tag: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("当前厂商「{studio}」与标签「{tag}」没有共同匹配的作品。"),
        Locale::Ja => format!("ブランド「{studio}」とタグ「{tag}」の両方に一致する作品がありません。"),
        Locale::En => format!("No titles match both studio \"{studio}\" and tag \"{tag}\"."),
        Locale::Ko => format!("제작사 '{studio}' 및 태그 '{tag}'에 동시에 일치하는 작품이 없습니다."),
    }
}

pub(crate) fn empty_studio_detail(studio: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("当前筛选厂商为「{studio}」，没有匹配的作品。点击下方按钮可恢复显示全部作品。"),
        Locale::Ja => format!("ブランド「{studio}」に一致する作品がありません。下のボタンで全作品を表示できます。"),
        Locale::En => format!("No titles match studio \"{studio}\". Click below to show all titles."),
        Locale::Ko => format!("제작사 '{studio}'에 일치하는 작품이 없습니다. 아래 버튼을 눌러 모든 작품을 표시하세요."),
    }
}

pub(crate) fn empty_tag_detail(tag: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("当前筛选标签为「{tag}」，没有匹配的作品。点击下方按钮可恢复显示全部作品。"),
        Locale::Ja => format!("タグ「{tag}」に一致する作品がありません。下のボタンで全作品を表示できます。"),
        Locale::En => format!("No titles match tag \"{tag}\". Click below to show all titles."),
        Locale::Ko => format!("태그 '{tag}'에 일치하는 작품이 없습니다. 아래 버튼을 눌러 모든 작품을 표시하세요."),
    }
}

pub(crate) fn empty_hidden_detail(hidden: usize) -> String {
    match locale() {
        Locale::ZhHans => format!("当前范围内共有 {hidden} 款已隐藏的游戏，在「•••」菜单中开启「显示已隐藏的游戏」即可查看。"),
        Locale::Ja => format!("この範囲には非表示のゲームが {hidden} 本あります。「•••」メニューで「非表示のゲームを表示」を有効にすると確認できます。"),
        Locale::En => format!("There are {hidden} hidden {} here. Enable \"Show Hidden Games\" in the \"•••\" menu to view them.", if hidden == 1 { "game" } else { "games" }),
        Locale::Ko => format!("이 범위에 숨겨진 게임이 {hidden}개 있습니다. '•••' 메뉴에서 '숨겨진 게임 표시'를 켜면 확인할 수 있습니다."),
    }
}

pub(crate) fn sidebar_title_with_bottle(page: &str, bottle: &str) -> String {
    format!("{page} · {bottle}")
}

pub(crate) fn rail_bottle_tooltip(bottle: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("{bottle} · 右键选择容器"),
        Locale::Ja => format!("{bottle} · 右クリックでコンテナを選択"),
        Locale::En => format!("{bottle} · Right-click to choose bottle"),
        Locale::Ko => format!("{bottle} · 우클릭하여 컨테이너 선택"),
    }
}

pub(crate) fn rail_tag_active_tooltip(tag: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("标签筛选：{tag} · 点击清除 / 右键切换"),
        Locale::Ja => format!("タグフィルター：{tag} · クリックで解除 / 右クリックで切替"),
        Locale::En => format!("Tag Filter: {tag} · Click to clear / Right-click to switch"),
        Locale::Ko => format!("태그 필터: {tag} · 클릭하여 해제 / 우클릭하여 전환"),
    }
}

pub(crate) fn rail_tag_count_tooltip(count: usize) -> String {
    match locale() {
        Locale::ZhHans => format!("标签（{count} 个）· 右键筛选"),
        Locale::Ja => format!("タグ（{count} 件）· 右クリックで絞り込み"),
        Locale::En => format!("Tags ({count}) · Right-click to filter"),
        Locale::Ko => format!("태그 ({count}개) · 우클릭하여 필터링"),
    }
}

pub(crate) fn back_to(destination: &str, shortcut: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("返回 {destination}（{shortcut}）"),
        Locale::Ja => format!("{destination} に戻る（{shortcut}）"),
        Locale::En => format!("Back to {destination} ({shortcut})"),
        Locale::Ko => format!("{destination}(으)로 돌아가기 ({shortcut})"),
    }
}

pub(crate) fn untagged_count(count: usize) -> String {
    match locale() {
        Locale::ZhHans => format!("未标记 ({count})"),
        Locale::Ja => format!("未分類 ({count})"),
        Locale::En => format!("Untagged ({count})"),
        Locale::Ko => format!("미분류 ({count})"),
    }
}

pub(crate) fn chip_studio(label: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("厂商: {label} ×"),
        Locale::Ja => format!("ブランド: {label} ×"),
        Locale::En => format!("Studio: {label} ×"),
        Locale::Ko => format!("제작사: {label} ×"),
    }
}

pub(crate) fn chip_tag(label: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("标签: {label} ×"),
        Locale::Ja => format!("タグ: {label} ×"),
        Locale::En => format!("Tag: {label} ×"),
        Locale::Ko => format!("태그: {label} ×"),
    }
}

pub(crate) fn title_studio(label: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("厂商：{label}"),
        Locale::Ja => format!("ブランド：{label}"),
        Locale::En => format!("Studio: {label}"),
        Locale::Ko => format!("제작사: {label}"),
    }
}

pub(crate) fn title_tag(label: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("标签：{label}"),
        Locale::Ja => format!("タグ：{label}"),
        Locale::En => format!("Tag: {label}"),
        Locale::Ko => format!("태그: {label}"),
    }
}

pub(crate) fn container_prefix(name: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("容器：{name}"),
        Locale::Ja => format!("コンテナ：{name}"),
        Locale::En => format!("Bottle: {name}"),
        Locale::Ko => format!("컨테이너: {name}"),
    }
}

pub(crate) fn view_all_games_in_library(count: usize) -> String {
    match locale() {
        Locale::ZhHans => format!("在游戏库查看全部（{count}）"),
        Locale::Ja => format!("ライブラリですべて表示（{count}）"),
        Locale::En => format!("View All in Library ({count})"),
        Locale::Ko => format!("라이브러리에서 모두 보기 ({count})"),
    }
}

pub(crate) fn value_inactive(title: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("{title}（未生效）"),
        Locale::Ja => format!("{title}（無効）"),
        Locale::En => format!("{title} (Inactive)"),
        Locale::Ko => format!("{title} (비활성)"),
    }
}

pub(crate) fn value_requires_confirm(title: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("{title}（需确认）"),
        Locale::Ja => format!("{title}（要確認）"),
        Locale::En => format!("{title} (Confirm)"),
        Locale::Ko => format!("{title} (확인 필요)"),
    }
}

pub(crate) fn engine_unavailable_item(name: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("{name} · 不可用"),
        Locale::Ja => format!("{name} · 利用不可"),
        Locale::En => format!("{name} · Unavailable"),
        Locale::Ko => format!("{name} · 사용 불가"),
    }
}

pub(crate) fn engine_unsupported_reason(option: &str, reason: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("{option}：{reason}"),
        Locale::Ja => format!("{option}：{reason}"),
        Locale::En => format!("{option}: {reason}"),
        Locale::Ko => format!("{option}: {reason}"),
    }
}

pub(crate) fn config_number_required_empty(title: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("请为“{title}”输入一个整数"),
        Locale::Ja => format!("「{title}」には整数を入力してください"),
        Locale::En => format!("Please enter an integer for \"{title}\""),
        Locale::Ko => format!("'{title}'에 정수를 입력하세요"),
    }
}

pub(crate) fn config_number_invalid(title: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("“{title}”需要一个整数"),
        Locale::Ja => format!("「{title}」には整数が必要です"),
        Locale::En => format!("\"{title}\" requires an integer"),
        Locale::Ko => format!("'{title}'에는 정수가 필요합니다"),
    }
}

pub(crate) fn config_number_range(title: &str, min: i64, max: i64) -> String {
    match locale() {
        Locale::ZhHans => format!("“{title}”必须在 {min} 到 {max} 之间"),
        Locale::Ja => format!("「{title}」は {min} 〜 {max} の範囲で指定してください"),
        Locale::En => format!("\"{title}\" must be between {min} and {max}"),
        Locale::Ko => format!("'{title}'은(는) {min}에서 {max} 사이여야 합니다"),
    }
}

pub(crate) fn config_not_text(title: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("“{title}”不是文本字段"),
        Locale::Ja => format!("「{title}」はテキスト項目ではありません"),
        Locale::En => format!("\"{title}\" is not a text field"),
        Locale::Ko => format!("'{title}'은(는) 텍스트 필드가 아닙니다"),
    }
}

pub(crate) fn import_added_count(count: usize) -> String {
    match locale() {
        Locale::ZhHans => format!("本次新增 {count} 个容器"),
        Locale::Ja => format!("{count} 件のコンテナを追加しました"),
        Locale::En => format!(
            "Added {count} {}",
            if count == 1 { "bottle" } else { "bottles" }
        ),
        Locale::Ko => format!("컨테이너 {count}개를 추가했습니다"),
    }
}

pub(crate) fn import_selected_button(count: usize) -> String {
    match locale() {
        Locale::ZhHans => format!("导入选中项（{count}）"),
        Locale::Ja => format!("選択項目をインポート（{count}）"),
        Locale::En => format!("Import Selected ({count})"),
        Locale::Ko => format!("선택 항목 가져오기 ({count})"),
    }
}

pub(crate) fn winetricks_preparing(verb: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("正在准备 {verb}…"),
        Locale::Ja => format!("{verb} を準備中…"),
        Locale::En => format!("Preparing {verb}…"),
        Locale::Ko => format!("{verb} 준비 중…"),
    }
}

pub(crate) fn winetricks_installing(verb: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("正在安装 {verb}…"),
        Locale::Ja => format!("{verb} をインストール中…"),
        Locale::En => format!("Installing {verb}…"),
        Locale::Ko => format!("{verb} 설치 중…"),
    }
}

pub(crate) fn winetricks_installed(verb: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("已安装 {verb}"),
        Locale::Ja => format!("{verb} をインストール済み"),
        Locale::En => format!("Installed {verb}"),
        Locale::Ko => format!("{verb} 설치됨"),
    }
}

pub(crate) fn winetricks_failed(verb: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("安装失败：{verb}"),
        Locale::Ja => format!("インストール失敗：{verb}"),
        Locale::En => format!("Failed: {verb}"),
        Locale::Ko => format!("설치 실패: {verb}"),
    }
}

pub(crate) fn winetricks_count_summary(installed: usize, total: usize) -> String {
    match locale() {
        Locale::ZhHans => format!("已安装 {installed} 个 · 共 {total} 个"),
        Locale::Ja => format!("インストール済み {installed} 件 · 合計 {total} 件"),
        Locale::En => format!("{installed} installed · {total} total"),
        Locale::Ko => format!("설치됨 {installed}개 · 전체 {total}개"),
    }
}

pub(crate) fn bytes_human(bytes: u64) -> String {
    match locale() {
        Locale::ZhHans => format!("{bytes} 字节"),
        Locale::Ja => format!("{bytes} バイト"),
        Locale::En => format!("{bytes} bytes"),
        Locale::Ko => format!("{bytes}바이트"),
    }
}

pub(crate) fn notice_launching_with_backend(name: &str, backend: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("正在启动 {name} · 图形后端：{backend}"),
        Locale::Ja => format!("{name} を起動中 · グラフィックス：{backend}"),
        Locale::En => format!("Launching {name} · Graphics: {backend}"),
        Locale::Ko => format!("{name} 실행 중 · 그래픽 백엔드: {backend}"),
    }
}

pub(crate) fn notice_launching(name: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("正在启动 {name}"),
        Locale::Ja => format!("{name} を起動中"),
        Locale::En => format!("Launching {name}"),
        Locale::Ko => format!("{name} 실행 중"),
    }
}

pub(crate) fn notice_launching_entry(title: &str, exe_name: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("正在启动 {title} · {exe_name}"),
        Locale::Ja => format!("{title} を起動中 · {exe_name}"),
        Locale::En => format!("Launching {title} · {exe_name}"),
        Locale::Ko => format!("{title} 실행 중 · {exe_name}"),
    }
}

pub(crate) fn notice_launch_failed(name: &str, reason: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("启动 {name} 失败：{reason}"),
        Locale::Ja => format!("{name} の起動に失敗しました：{reason}"),
        Locale::En => format!("Failed to launch {name}: {reason}"),
        Locale::Ko => format!("{name} 실행 실패: {reason}"),
    }
}

pub(crate) fn notice_metadata_unsaved(error: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("资料来源未保存：{error}"),
        Locale::Ja => format!("情報ソース設定を保存できませんでした：{error}"),
        Locale::En => format!("Metadata preferences not saved: {error}"),
        Locale::Ko => format!("메타데이터 소스를 저장하지 못했습니다: {error}"),
    }
}

pub(crate) fn notice_tag_renamed(old_tag: &str, new_tag: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("已将标签「{old_tag}」重命名为「{new_tag}」"),
        Locale::Ja => format!("タグ「{old_tag}」の名前を「{new_tag}」に変更しました"),
        Locale::En => format!("Renamed tag \"{old_tag}\" to \"{new_tag}\""),
        Locale::Ko => format!("태그 '{old_tag}'을(를) '{new_tag}'(으)로 변경했습니다"),
    }
}

pub(crate) fn notice_tag_deleted(tag: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("已删除标签「{tag}」"),
        Locale::Ja => format!("タグ「{tag}」を削除しました"),
        Locale::En => format!("Deleted tag \"{tag}\""),
        Locale::Ko => format!("태그 '{tag}'을(를) 삭제했습니다"),
    }
}

pub(crate) fn notice_bottle_created(name: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("已成功创建容器 {name}"),
        Locale::Ja => format!("コンテナ {name} を作成しました"),
        Locale::En => format!("Created bottle {name}"),
        Locale::Ko => format!("컨테이너 {name}을(를) 생성했습니다"),
    }
}

pub(crate) fn notice_winetricks_failed(count: usize, reason: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("{count} 个组件安装失败：{reason}"),
        Locale::Ja => format!("{count} 件のコンポーネントのインストールに失敗しました：{reason}"),
        Locale::En => format!("{count} component(s) failed to install: {reason}"),
        Locale::Ko => format!("{count}개 구성 요소 설치 실패: {reason}"),
    }
}

pub(crate) fn notice_program_saved(name: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("已保存 {name} 的程序设置"),
        Locale::Ja => format!("{name} のプログラム設定を保存しました"),
        Locale::En => format!("Saved program settings for {name}"),
        Locale::Ko => format!("{name}의 프로그램 설정을 저장했습니다"),
    }
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

pub(crate) fn notice_opened_tool(label: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("已打开 {label}"),
        Locale::Ja => format!("{label} を開きました"),
        Locale::En => format!("Opened {label}"),
        Locale::Ko => format!("{label}을(를) 열었습니다"),
    }
}

pub(crate) fn notice_diagnostics_exported(path: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("诊断报告已导出到 {path}"),
        Locale::Ja => format!("診断レポートを {path} にエクスポートしました"),
        Locale::En => format!("Diagnostics report exported to {path}"),
        Locale::Ko => format!("진단 보고서를 {path}(으)로 내보냈습니다"),
    }
}

pub(crate) fn notice_checks_with_failures(failures: usize) -> String {
    match locale() {
        Locale::ZhHans => format!("自动检查完成：{failures} 项需要关注"),
        Locale::Ja => format!("自動チェック完了：{failures} 件の確認項目があります"),
        Locale::En => format!("Checks complete: {failures} issue(s) need attention"),
        Locale::Ko => format!("자동 검사 완료: 확인 필요한 항목 {failures}개"),
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

pub(crate) fn notice_wine_update_available(remote: &str, local: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("发现新版 Wine {remote}（当前 {local}），可在设置中更新运行时"),
        Locale::Ja => format!("新しい Wine {remote} が見つかりました（現在 {local}）。設定から更新できます"),
        Locale::En => format!("New Wine version {remote} available (current: {local}). Update in Settings."),
        Locale::Ko => format!("새 Wine 버전 {remote} 발견(현재: {local}). 설정에서 업데이트할 수 있습니다."),
    }
}

pub(crate) fn notice_import_failed(reason: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("导入失败：{reason}"),
        Locale::Ja => format!("インポート失敗：{reason}"),
        Locale::En => format!("Import failed: {reason}"),
        Locale::Ko => format!("가져오기 실패: {reason}"),
    }
}

pub(crate) fn notice_remove_failed(reason: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("移除失败：{reason}"),
        Locale::Ja => format!("削除失敗：{reason}"),
        Locale::En => format!("Remove failed: {reason}"),
        Locale::Ko => format!("제거 실패: {reason}"),
    }
}

pub(crate) fn notice_cannot_run_file(name: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("无法运行 {name}：请选择 Windows 程序或安装包"),
        Locale::Ja => format!("{name} を実行できません：Windows プログラムまたはインストーラーを選択してください"),
        Locale::En => format!("Cannot run {name}: please select a Windows executable or installer"),
        Locale::Ko => format!("{name}을(를) 실행할 수 없습니다: Windows 프로그램이나 설치 파일을 선택하세요"),
    }
}

pub(crate) fn notice_clear_cache_failed(error: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("清除缓存失败：{error}"),
        Locale::Ja => format!("キャッシュの削除に失敗しました：{error}"),
        Locale::En => format!("Failed to clear cache: {error}"),
        Locale::Ko => format!("캐시 삭제 실패: {error}"),
    }
}

pub(crate) fn duplicate_env_key(key: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("环境变量 {key} 重复"),
        Locale::Ja => format!("環境変数 {key} が重複しています"),
        Locale::En => format!("Duplicate environment variable: {key}"),
        Locale::Ko => format!("중복된 환경 변수: {key}"),
    }
}

pub(crate) fn engine_fallback(effective: &str, from: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("{effective}（{from} 不可用，已回退）"),
        Locale::Ja => format!("{effective}（{from} 利用不可のためフォールバック）"),
        Locale::En => format!("{effective} ({from} unavailable, fell back)"),
        Locale::Ko => format!("{effective} ({from} 사용 불가, 대체됨)"),
    }
}

pub(crate) fn dialog_save_failed(error: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("保存失败：{error}"),
        Locale::Ja => format!("保存失敗：{error}"),
        Locale::En => format!("Save failed: {error}"),
        Locale::Ko => format!("저장 실패: {error}"),
    }
}

pub(crate) fn clipboard_prompt_body(content: &str, size: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("启动前检测到剪贴板中有 {content}（{size}）。Wine 可能读取剪贴板，请选择如何处理。"),
        Locale::Ja => format!("起動前にクリップボード内の {content}（{size}）を検出しました。Wine が読み取る可能性があります。操作を選択してください。"),
        Locale::En => format!("Detected {content} ({size}) in the clipboard before launch. Wine may read it — choose how to proceed."),
        Locale::Ko => format!("실행 전 클립보드에서 {content}({size})이(가) 감지되었습니다. Wine이 이를 읽을 수 있으므로 처리 방법을 선택하세요."),
    }
}

pub(crate) fn legacy_fullscreen_body(program_name: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("「{program_name}」上次运行时请求切换显示模式被 Wine 拒绝，全屏可能未生效。需要低分辨率全屏的旧游戏可通过「旧版全屏兼容」修复；该开关仅作用于此程序。"),
        Locale::Ja => format!("「{program_name}」の前回実行時、ディスプレイモード変更要求が Wine に拒否されたため全画面が正しく機能しなかった可能性があります。低解像度全画面を必要とする旧作ゲームは「レガシー全画面互換」で修正できます（このプログラムにのみ適用）。"),
        Locale::En => format!("On the last run, \"{program_name}\" requested a display mode change that Wine rejected, so fullscreen may not have worked. Older games needing low-resolution fullscreen can enable Legacy Fullscreen Compatibility (applies only to this program)."),
        Locale::Ko => format!("지난 실행 시 '{program_name}'의 디스플레이 모드 전환 요청이 Wine에서 거부되어 전체 화면이 적용되지 않았을 수 있습니다. 저해상도 전체 화면이 필요한 구형 게임은 '레거시 전체 화면 호환'으로 해결할 수 있습니다(이 프로그램에만 적용)."),
    }
}

pub(crate) fn bottle_rename_prompt(name: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("为「{name}」输入新的名称。"),
        Locale::Ja => format!("「{name}」の新しい名前を入力してください。"),
        Locale::En => format!("Enter a new name for \"{name}\"."),
        Locale::Ko => format!("'{name}'의 새 이름을 입력하세요."),
    }
}

pub(crate) fn bottle_duplicate_prompt(name: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("将「{name}」克隆到同一位置，并复制全部设置与已安装程序。"),
        Locale::Ja => format!("「{name}」を同じ場所に複製し、設定とインストール済みプログラムをすべてコピーします。"),
        Locale::En => format!("Clone \"{name}\" in the same location, copying all settings and installed programs."),
        Locale::Ko => format!("'{name}'을(를) 같은 위치에 복제하고 모든 설정과 설치된 프로그램을 복사합니다."),
    }
}

pub(crate) fn bottle_remove_confirm(name: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("确定要移除「{name}」吗？"),
        Locale::Ja => format!("「{name}」を削除してもよろしいですか？"),
        Locale::En => format!("Remove \"{name}\"?"),
        Locale::Ko => format!("'{name}'을(를) 제거하시겠습니까?"),
    }
}

pub(crate) fn setup_ready_version(version: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("Wine 运行时 {version} 已就绪"),
        Locale::Ja => format!("Wine ランタイム {version} の準備ができました"),
        Locale::En => format!("Wine runtime {version} is ready"),
        Locale::Ko => format!("Wine 런타임 {version} 준비 완료"),
    }
}

pub(crate) fn bridge_failed_detail(reason: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("{reason}\n\n在重新构建或修复桥接辅助进程前，重试可能无效。"),
        Locale::Ja => format!("{reason}\n\nブリッジヘルパーを再ビルドまたは修復するまで、再試行しても解決しない場合があります。"),
        Locale::En => format!("{reason}\n\nRetrying may not succeed until the bridge helper is rebuilt or repaired."),
        Locale::Ko => format!("{reason}\n\n브리지 도우미를 다시 빌드하거나 복구하기 전에는 재시도해도 해결되지 않을 수 있습니다."),
    }
}

pub(crate) fn bridge_connected_detail(helper: &str, version: &str, methods: usize) -> String {
    match locale() {
        Locale::ZhHans => format!("{helper}\n{version} · {methods} 个接口"),
        Locale::Ja => format!("{helper}\n{version} · {methods} メソッド"),
        Locale::En => format!("{helper}\n{version} · {methods} methods"),
        Locale::Ko => format!("{helper}\n{version} · {methods}개 메서드"),
    }
}

pub(crate) fn engine_feature_confirm_question(feature: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("确定要启用「{feature}」吗？"),
        Locale::Ja => format!("「{feature}」を有効にしてもよろしいですか？"),
        Locale::En => format!("Enable \"{feature}\"?"),
        Locale::Ko => format!("'{feature}'을(를) 활성화하시겠습니까?"),
    }
}

pub(crate) fn engine_switch_question(engine_name: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("将容器切换到「{engine_name}」？"),
        Locale::Ja => format!("コンテナを「{engine_name}」に切り替えますか？"),
        Locale::En => format!("Switch bottle to \"{engine_name}\"?"),
        Locale::Ko => format!("컨테이너를 '{engine_name}'(으)로 전환하시겠습니까?"),
    }
}

pub(crate) fn other_game_capturing_warning(game: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("正在捕获 {game}；请先停止该会话，再开始当前游戏的捕获。"),
        Locale::Ja => format!("{game} をキャプチャ中です。現在のゲームを開始する前にそのセッションを停止してください。"),
        Locale::En => format!("Currently capturing {game}. Stop that session before capturing this game."),
        Locale::Ko => format!("{game}을(를) 캡처 중입니다. 현재 게임을 시작하기 전에 해당 세션을 중지하세요."),
    }
}

pub(crate) fn hud_capture_badge(badge: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("捕获 · {badge}"),
        Locale::Ja => format!("キャプチャ · {badge}"),
        Locale::En => format!("Capture · {badge}"),
        Locale::Ko => format!("캡처 · {badge}"),
    }
}

pub(crate) fn capture_active_session(title: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("当前活动会话：{title}"),
        Locale::Ja => format!("現在のアクティブセッション：{title}"),
        Locale::En => format!("Active session: {title}"),
        Locale::Ko => format!("현재 활성 세션: {title}"),
    }
}

pub(crate) fn capture_auto_bound(count: usize) -> String {
    match locale() {
        Locale::ZhHans => format!("✓ 已自动恢复 {count} 个线程绑定"),
        Locale::Ja => format!("✓ {count} 件のスレッド割り当てを自動復元しました"),
        Locale::En => format!("✓ Auto-restored {count} thread binding(s)"),
        Locale::Ko => format!("✓ 스레드 바인딩 {count}개 자동 복원됨"),
    }
}

pub(crate) fn capture_status_line(status: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("捕获：{status}"),
        Locale::Ja => format!("キャプチャ：{status}"),
        Locale::En => format!("Capture: {status}"),
        Locale::Ko => format!("캡처: {status}"),
    }
}

pub(crate) fn translation_status_line(status: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("翻译：{status}"),
        Locale::Ja => format!("翻訳：{status}"),
        Locale::En => format!("Translation: {status}"),
        Locale::Ko => format!("번역: {status}"),
    }
}

pub(crate) fn speaker_prefix(speaker: &str) -> String {
    match locale() {
        Locale::ZhHans => format!("说话人：{speaker}"),
        Locale::Ja => format!("話者：{speaker}"),
        Locale::En => format!("Speaker: {speaker}"),
        Locale::Ko => format!("화자: {speaker}"),
    }
}

pub(crate) fn threads_section_title(shown: usize, total: usize) -> String {
    match locale() {
        Locale::ZhHans => format!("文本线程与 Hook 注入（{shown}/{total}）"),
        Locale::Ja => format!("テキストスレッドと Hook 注入（{shown}/{total}）"),
        Locale::En => format!("Text Threads & Hook Injection ({shown}/{total})"),
        Locale::Ko => format!("텍스트 스레드 및 Hook 주입 ({shown}/{total})"),
    }
}

pub(crate) fn hidden_empty_threads(count: usize) -> String {
    match locale() {
        Locale::ZhHans => format!("已隐藏 {count} 条空预览"),
        Locale::Ja => format!("空のプレビュー {count} 件を非表示"),
        Locale::En => format!("{count} empty hidden"),
        Locale::Ko => format!("빈 미리보기 {count}개 숨김"),
    }
}

pub(crate) fn hidden_empty_threads_hint(count: usize) -> String {
    match locale() {
        Locale::ZhHans => format!("当前有 {count} 条空预览线程被隐藏，点击「仅看有文本」可查看全部。"),
        Locale::Ja => format!("空のプレビューのスレッドが {count} 件非表示です。「テキストありのみ」を切り替えるとすべて表示できます。"),
        Locale::En => format!("{count} empty preview thread(s) hidden. Toggle \"Text only\" to view all."),
        Locale::Ko => format!("빈 미리보기 스레드 {count}개가 숨겨져 있습니다. '텍스트만 보기'를 끄면 모두 볼 수 있습니다."),
    }
}

pub(crate) fn hidden_empty_threads_search_hint(count: usize) -> String {
    match locale() {
        Locale::ZhHans => format!("没有匹配当前搜索条件的可见线程（另有 {count} 条空预览线程被「仅看有文本」隐藏）。"),
        Locale::Ja => format!("検索条件に一致する表示スレッドがありません（別に空のスレッド {count} 件が非表示です）。"),
        Locale::En => format!("No visible threads match the search ({count} empty thread(s) hidden by \"Text only\")."),
        Locale::Ko => format!("검색 조건과 일치하는 스레드가 없습니다(빈 스레드 {count}개가 '텍스트만 보기'로 숨겨짐)."),
    }
}

pub(crate) fn history_section_title(shown: usize, total: usize) -> String {
    match locale() {
        Locale::ZhHans => format!("会话历史（{shown}/{total}）"),
        Locale::Ja => format!("セッション履歴（{shown}/{total}）"),
        Locale::En => format!("Session History ({shown}/{total})"),
        Locale::Ko => format!("세션 기록 ({shown}/{total})"),
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

pub(crate) fn thread_bindings_summary(total: usize, body: usize, speaker: usize, choice: usize) -> String {
    match locale() {
        Locale::ZhHans => format!("线程绑定 {total}（正文 {body} · 说话人 {speaker} · 选项 {choice}）"),
        Locale::Ja => format!("スレッド割当 {total}（本文 {body} · 話者 {speaker} · 選択肢 {choice}）"),
        Locale::En => format!("Bindings {total} (Body {body} · Speaker {speaker} · Choice {choice})"),
        Locale::Ko => format!("스레드 바인딩 {total} (본문 {body} · 화자 {speaker} · 선택지 {choice})"),
    }
}

pub(crate) fn custom_hooks_count(count: usize) -> String {
    match locale() {
        Locale::ZhHans => format!("自定义 Hook {count}"),
        Locale::Ja => format!("カスタム Hook {count}"),
        Locale::En => format!("Custom Hooks {count}"),
        Locale::Ko => format!("사용자 지정 Hook {count}"),
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

pub(crate) fn eta_seconds(secs: u64) -> String {
    match locale() {
        Locale::ZhHans => format!("预计剩余约 {secs} 秒"),
        Locale::Ja => format!("残り約 {secs} 秒"),
        Locale::En => format!("About {secs}s remaining"),
        Locale::Ko => format!("약 {secs}초 남음"),
    }
}

pub(crate) fn eta_minutes(mins: u64) -> String {
    match locale() {
        Locale::ZhHans => format!("预计剩余约 {mins} 分钟"),
        Locale::Ja => format!("残り約 {mins} 分"),
        Locale::En => format!("About {mins}m remaining"),
        Locale::Ko => format!("약 {mins}분 남음"),
    }
}

pub(crate) fn eta_minutes_seconds(mins: u64, secs: u64) -> String {
    match locale() {
        Locale::ZhHans => format!("预计剩余约 {mins} 分 {secs} 秒"),
        Locale::Ja => format!("残り約 {mins} 分 {secs} 秒"),
        Locale::En => format!("About {mins}m {secs}s remaining"),
        Locale::Ko => format!("약 {mins}분 {secs}초 남음"),
    }
}

pub(crate) fn eta_hours_minutes(hours: u64, mins: u64) -> String {
    match locale() {
        Locale::ZhHans => format!("预计剩余约 {hours} 小时 {mins} 分"),
        Locale::Ja => format!("残り約 {hours} 時間 {mins} 分"),
        Locale::En => format!("About {hours}h {mins}m remaining"),
        Locale::Ko => format!("약 {hours}시간 {mins}분 남음"),
    }
}

pub(crate) fn range_error(label: &str, min: i64, max: i64) -> String {
    match locale() {
        Locale::ZhHans => format!("{label} 必须在 {min} 到 {max} 之间"),
        Locale::Ja => format!("{label} は {min} 〜 {max} の範囲で入力してください"),
        Locale::En => format!("{label} must be between {min} and {max}"),
        Locale::Ko => format!("{label}은(는) {min}에서 {max} 사이여야 합니다"),
    }
}

pub(crate) fn tag_count_works(count: usize) -> String {
    match locale() {
        Locale::ZhHans => format!("{count} 部作品"),
        Locale::Ja => format!("{count} 本の作品"),
        Locale::En => format!("{count} {}", if count == 1 { "title" } else { "titles" }),
        Locale::Ko => format!("{count}개 작품"),
    }
}

pub(crate) fn character_count(count: usize) -> String {
    match locale() {
        Locale::ZhHans => format!("{count} 位"),
        Locale::Ja => format!("{count} 人"),
        Locale::En => format!("{count}"),
        Locale::Ko => format!("{count}명"),
    }
}

pub(crate) fn character_view_all(count: usize) -> String {
    match locale() {
        Locale::ZhHans => format!("查看全部 {count} 位"),
        Locale::Ja => format!("すべて表示（{count} 人）"),
        Locale::En => format!("Show All ({count})"),
        Locale::Ko => format!("모두 보기 ({count}명)"),
    }
}
