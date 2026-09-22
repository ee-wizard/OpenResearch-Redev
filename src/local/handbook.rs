//! Editable research/writing handbook.
//!
//! Chapters live as markdown files under `<data_dir>/handbook/` with an
//! `index.json` that defines their order. The dashboard renders them as a
//! chapter-based guide that teams can customize.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{anyhow, Result};
use crate::store;

/// One chapter as exposed to the UI.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chapter {
    pub id: String,
    pub title: String,
    pub file_path: PathBuf,
}

/// On-disk chapter metadata entry.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChapterIndexEntry {
    id: String,
    title: String,
    file: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChapterIndex {
    chapters: Vec<ChapterIndexEntry>,
}

/// `<data_dir>/handbook`
pub fn handbook_dir() -> PathBuf {
    store::data_dir().join("handbook")
}

/// Idempotently seed the default handbook chapters.
pub fn ensure_handbook() -> Result<()> {
    let dir = handbook_dir();
    std::fs::create_dir_all(&dir)?;

    let defaults = default_chapters();
    let index_path = dir.join("index.json");
    if !index_path.exists() {
        let index = ChapterIndex {
            chapters: defaults
                .iter()
                .map(|(id, title, file, _)| ChapterIndexEntry {
                    id: (*id).to_string(),
                    title: (*title).to_string(),
                    file: (*file).to_string(),
                })
                .collect(),
        };
        std::fs::write(&index_path, serde_json::to_string_pretty(&index)?)?;
    }

    for (_id, title, file, body) in defaults {
        let path = dir.join(file);
        if !path.exists() {
            let content = format!("---\ntitle: {}\n---\n\n{}", title, body);
            std::fs::write(&path, content)?;
        }
    }

    Ok(())
}

/// Return the ordered list of chapters from `index.json`, falling back to
/// scanning the directory if the index is missing.
pub fn list_chapters() -> Result<Vec<Chapter>> {
    ensure_handbook()?;
    let dir = handbook_dir();
    let index_path = dir.join("index.json");

    let entries: Vec<ChapterIndexEntry> = if index_path.exists() {
        let text = std::fs::read_to_string(&index_path)?;
        serde_json::from_str(&text).unwrap_or_default()
    } else {
        Vec::new()
    };

    let mut chapters = Vec::new();
    for entry in entries {
        let path = dir.join(&entry.file);
        let title = if path.exists() {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|content| parse_title_from_frontmatter(&content))
                .unwrap_or(entry.title)
        } else {
            entry.title
        };
        chapters.push(Chapter {
            id: entry.id,
            title,
            file_path: path,
        });
    }

    if chapters.is_empty() {
        // Fallback: scan markdown files when no index exists yet.
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("md") {
                continue;
            }
            let id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let title = std::fs::read_to_string(&path)
                .ok()
                .and_then(|content| parse_title_from_frontmatter(&content))
                .unwrap_or_else(|| id.clone());
            chapters.push(Chapter {
                id,
                title,
                file_path: path,
            });
        }
        chapters.sort_by(|a, b| a.file_path.cmp(&b.file_path));
    }

    Ok(chapters)
}

/// Read the markdown content of a chapter by id.
pub fn get_chapter(id: &str) -> Result<String> {
    ensure_handbook()?;
    let chapter = find_chapter(id)?;
    std::fs::read_to_string(&chapter.file_path)
        .map_err(|e| anyhow!("Could not read chapter {}: {}", chapter.file_path.display(), e))
}

/// Write new markdown content for a chapter and update its title in
/// `index.json` when a frontmatter title is present.
pub fn save_chapter(id: &str, content: &str) -> Result<()> {
    ensure_handbook()?;
    let chapter = find_chapter(id)?;
    std::fs::write(&chapter.file_path, content)?;

    if let Some(new_title) = parse_title_from_frontmatter(content) {
        let dir = handbook_dir();
        let index_path = dir.join("index.json");
        if index_path.exists() {
            let text = std::fs::read_to_string(&index_path)?;
            if let Ok(mut index) = serde_json::from_str::<ChapterIndex>(&text) {
                for entry in &mut index.chapters {
                    if entry.id == id {
                        entry.title = new_title;
                        break;
                    }
                }
                std::fs::write(&index_path, serde_json::to_string_pretty(&index)?)?;
            }
        }
    }

    Ok(())
}

fn find_chapter(id: &str) -> Result<Chapter> {
    list_chapters()?
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| anyhow!("Chapter {id} not found"))
}

fn parse_title_from_frontmatter(content: &str) -> Option<String> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let after_open = content
        .strip_prefix("---\n")
        .or_else(|| content.strip_prefix("---\r\n"))?;
    let end = after_open.find("\n---")?;
    let block = &after_open[..end];
    for line in block.lines() {
        if let Some(value) = line.strip_prefix("title:") {
            return Some(
                value
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string(),
            );
        }
    }
    None
}

fn default_chapters() -> Vec<(&'static str, &'static str, &'static str, &'static str)> {
    vec![
        (
            "01-macro",
            "第一章：宏观认识",
            "01-macro.md",
            r#"# 第一章：宏观认识 — 从审稿人视角看论文质量

一篇高质量的科研论文通常需要在四个维度上同时达标。借用审稿人常用的总结方式，可以概括为 **4N**：**Novel Problem**、**Novel Method**、**Nice Story**、**Nice Presentation**。

## 1.1 Novel Problem：问题的新颖性

- 是否提出了一个此前没有被充分定义或解决的研究问题？
- 该问题是否对社区有实际意义，而不仅仅是“为了不同而不同”？
- 是否清晰地说明了问题的边界、输入输出和评估标准？

好的问题往往来自对真实研究痛点的观察。写作时要在 Introduction 的前两段就让读者相信：这个问题值得被解决，而且现有方法没有解决好。

## 1.2 Novel Method：方法的新颖性

- 方法是否在技术上有非平凡的贡献？
- 与最相关的基线相比，核心差异是什么？
- 是否给出了足够的理论直觉或实验证据来支撑有效性？

方法的新颖性不等同于复杂性。有时候一个简洁的洞察（simple but effective）比复杂的公式堆叠更有价值。

## 1.3 Nice Story：故事的完整性

- 从 Motivation 到 Method 再到 Experiments 是否逻辑自洽？
- 每一节是否都在回答一个读者会自然提出的问题？
- 论文的“主线”是否清晰，辅助实验是否服务于主线？

故事线决定了论文是否容易被理解和记住。建议在动笔前先画出论文的“逻辑链”：背景 → 缺口 → 思路 → 验证 → 结论。

## 1.4 Nice Presentation：表达的清晰度

- 符号、术语是否前后一致？
- 图表是否自解释（self-contained）？
- 公式、算法、实验设置是否足够详细以复现？

Presentation 是审稿人对论文的第一印象。再扎实的工作，如果表达混乱，也容易被低估。

## 1.5 4N 检查清单

- [ ] 我在 Introduction 中明确指出了要解决的缺口。
- [ ] 我的方法与现有工作的核心差异不超过三句话能说清。
- [ ] 实验设计直接验证了我提出的核心假设。
- [ ] 图表不看正文也能大致理解结论。
- [ ] 全文术语、符号、图表风格保持一致。
"#,
        ),
        (
            "02-idea",
            "第二章：Idea构思",
            "02-idea.md",
            r#"# 第二章：Idea构思

Idea 不是凭空出现的，它有其生命周期，也有一系列可训练的思维框架。

## 2.1 Idea 的生命周期

一个 Idea 从产生到成熟通常会经历以下阶段：

1. **观察（Observation）**：注意到一个反复出现的现象、痛点或异常结果。
2. **抽象（Abstraction）**：把现象提炼成一个可研究的问题或假设。
3. **验证（Validation）**：通过小规模实验、理论分析或案例研究判断可行性。
4. **迭代（Iteration）**：根据反馈调整假设和方法。
5. **封装（Packaging）**：把成果组织成可发表的论文或产品。

很多初学者卡在第一步，以为必须有“惊天动地”的灵感。实际上，大多数好的 Idea 都来自对已有工作的细心阅读和质疑。

## 2.2 5维思考框架

当你有一个模糊方向时，可以从以下五个维度扩展它：

- **更高（Higher）**：提升性能上限，例如更高的准确率、更低的误差。
- **更快（Faster）**：降低计算或推理成本，例如更高效的算法、更轻量的模型。
- **更强（Stronger）**：增强鲁棒性、泛化性或安全性。
- **更省（Cheaper）**：减少数据、标注、算力或人力成本。
- **更广（Broader）**：把方法扩展到更多任务、模态或应用场景。

一个 Idea 往往可以先沿一个维度切入，再逐步扩展到其他维度。例如“让大模型在特定任务上更快”可以进一步变成“在更快的同时保持更高准确率，并支持更多数据模态”。

## 2.3 颠覆式创新 vs. 渐进式创新

- **颠覆式创新（Disruptive）**：改变问题的基本假设或范式。风险高、影响大、审稿人评价两极分化。
- **渐进式创新（Incremental）**：在现有框架内做出稳定改进。更容易被接受，但也更难脱颖而出。

对于早期研究者，建议以渐进式创新积累可信度，同时保留一两个“颠覆式”的长期问题在心中。写作时，即使是渐进式工作，也要把它的独特洞察放大，而不是罗列实验结果。

## 2.4 Idea 记录模板

建议用一页纸记录每个 Idea：

- **问题一句话**：我们要解决什么？
- **动机**：为什么现在重要？
- **关键假设**：如果 X 成立，我们就能得到 Y。
- **最小验证**：一个周末能跑完的实验是什么？
- **风险**：最可能让 Idea 失败的地方在哪里？
- **故事线**：如果结果 positive，论文的主标题会是什么？
"#,
        ),
        (
            "03-writing",
            "第三章：论文写作",
            "03-writing.md",
            r#"# 第三章：论文写作

科研论文是一次有组织的说服：你要让读者相信，你提出的问题重要，你的方法有效，你的结论可靠。

## 3.1 论文全流程

1. **确定核心贡献**：在动笔前用一句话概括论文的卖点。
2. **搭建骨架**：先写标题、摘要、Introduction、方法大纲和实验大纲。
3. **填充图表**：把最重要的图和表先做出来，它们会反过来约束正文。
4. **迭代写作**：从粗糙的段落到精细的句子，不要追求第一稿完美。
5. **冷却修改**：放置几天后再读，容易发现逻辑漏洞和表达问题。
6. **同行评审**：找合作者或导师按审稿人视角通读。

## 3.2 Introduction 思考模型

一个好的 Introduction 通常遵循“漏斗式”结构：

- **大背景（1段）**：领域、重要性、读者为什么应该关心。
- **已有工作（1-2段）**：简述最相关的方法，为缺口做铺垫。
- **缺口/问题（1段）**：指出现有方法的局限，引出你的动机。
- **我们的方法（1-2段）**：用非技术语言概括核心思想。
- **贡献（1段）**：逐条列出具体贡献，对应后面的章节。

每一段的最后一句最好起到“承上启下”的作用，引导读者自然进入下一段。

## 3.3 技术类论文模板

适合提出新算法、新模型、新系统的工作：

- **Problem**：形式化问题定义，明确输入输出和假设。
- **Method**：分模块介绍，先给整体框架图，再讲每个组件。
- **Theory（可选）**：给出关键定理、收敛性、复杂度或 guarantees。
- **Experiments**：主实验 + 消融实验 + 案例分析 + 效率实验。
- **Limitations**：主动提及适用范围和未解决的问题。

## 3.4 Benchmark 类论文模板

适合提出新基准、新数据集、新评测的工作：

- **Motivation**：为什么现有 benchmark 不够？
- **Data/Task Design**：数据如何收集、清洗、标注？任务如何设计？
- **Metrics**：选择指标的理由，以及它们如何反映真实能力。
- **Baselines & Results**：覆盖主流方法，给出清晰的排行榜。
- **Analysis**：错误分析、难度分布、模型行为洞察。

## 3.5 写作 Checklist

- [ ] 标题是否准确概括了核心贡献？
- [ ] 摘要是否独立成篇，包含问题、方法、结果？
- [ ] Introduction 是否在前两页就让读者明白论文价值？
- [ ] 方法部分是否有清晰的算法或流程图？
- [ ] 每个实验图表都在正文中被明确引用和解释。
- [ ] Related Work 区分了“和我们类似”与“和我们不同”的工作。
- [ ] 结论没有引入新观点或新结果。
- [ ] 全文语法、标点和格式统一。
"#,
        ),
        (
            "04-figures",
            "第四章：科研作图",
            "04-figures.md",
            r#"# 第四章：科研作图

图是论文的“视觉摘要”。一张好的图能大幅降低理解门槛，也能让审稿人更快抓住你的核心信息。

## 4.1 三类核心图

### 动机图（Motivation Figure）

- 目的：让读者快速理解“为什么这个问题重要/困难”。
- 常用形式：示例输入输出、失败案例、与理想结果的对比。
- 要点：聚焦一个具体场景，避免信息过载。

### 总览图（Overview Figure）

- 目的：展示方法的整体结构或流程。
- 常用形式：模块框图、数据流图、pipeline 图。
- 要点：每个模块的输入输出清晰，箭头方向明确，配色统一。

### 实验图（Experiment Figure）

- 目的：用数据说话，支撑论文的核心结论。
- 常用形式：折线图、柱状图、热力图、散点图、表格。
- 要点：坐标轴标签、单位、误差棒、显著性标记缺一不可。

## 4.2 设计范式

1. **一张图一个信息**：不要试图把所有结果塞进一张图。
2. **自解释性**：图注 + 图中的文字应能让读者不看正文也懂大意。
3. **视觉层次**：最重要的信息用最大、最亮的元素呈现。
4. **一致性**：同篇论文中的颜色、字体、线宽、标记风格保持一致。
5. **可复用性**：使用脚本生成图，确保数据更新后图能自动重绘。

## 4.3 常用工具

- Python：matplotlib、seaborn、plotly
- 矢量图：Inkscape、Adobe Illustrator、Figma
- 示意图：draw.io、Excalidraw、TikZ
- 深度学习可视化：t-SNE / UMAP、attention heatmaps

## 4.4 绘图 Checklist

- [ ] 所有字体大小在论文缩放后仍然可读。
- [ ] 颜色对色盲读者友好（避免仅用红绿区分）。
- [ ] 图例位置不遮挡关键数据。
- [ ] 坐标轴有标签和单位。
- [ ] 误差棒或置信区间已标注。
- [ ] 图中使用的缩写已在图注中解释。
- [ ] 矢量图导出为 PDF/SVG，避免位图放大后模糊。
- [ ] 图的编号和正文引用顺序一致。
"#,
        ),
        (
            "05-practice",
            "第五章：前沿实战",
            "05-practice.md",
            r#"# 第五章：前沿实战 — Vibe Research/Coding/Figure/Writing

“Vibe”式科研强调人与 AI 的紧密协作：研究者负责方向、判断和故事，AI 负责快速执行、迭代和打磨。

## 5.1 Vibe Research

- **快速扫描**：用 AI 辅助阅读大量论文，提取方法、数据集、指标和结论。
- **缺口发现**：让 AI 对比多篇相关工作，列出尚未解决的问题。
- **假设生成**：基于观察到的模式，提出可验证的假设。
- **文献管理**：使用 Zotero、Obsidian 或 Notion 维护一个动态文献库。

## 5.2 Vibe Coding

- **从原型到生产**：先用 AI 生成最小可行实现（MVP），再逐步加固。
- **版本控制**：每个重要实验分支都使用 Git 管理，确保可回溯。
- **实验跟踪**：用结构化日志记录超参数、随机种子和关键结果。
- **自动化**：写好脚本让训练、评估、绘图一键运行。

## 5.3 Vibe Figure

- **草图先行**：在写代码前先手绘或文字描述想要的图。
- **模板复用**：建立个人/团队的图表模板，统一风格。
- **AI 辅助**：用 AI 生成 TikZ、matplotlib 草图或 SVG 示意图。
- **迭代打磨**：先完成 80% 的图，再根据反馈精细调整。

## 5.4 Vibe Writing

- **声音分离**：把“写内容”和“改语言”分成两个阶段。
- **逆向大纲**：先写段落标题和主题句，再填充细节。
- **结构化提示**：给 AI 明确的角色、目标和约束，例如“请把这段改得更像 ICML 风格”。
- **多轮润色**：先改逻辑流，再改句子，最后改语法和格式。

## 5.5 实战经验

- **小步快跑**：不要等所有实验都做完才开始写，写的过程会反过来指导实验。
- **保留失败记录**：负面结果往往比正面结果更能揭示问题边界。
- **定期复盘**：每周回顾一次 Idea 清单，淘汰低优先级的，集中资源做高潜力的。
- **建立反馈环**：尽早把草稿给导师、同事或 AI 审阅，收集外部视角。
"#,
        ),
        (
            "06-cases",
            "第六章：顶会案例",
            "06-cases.md",
            r#"# 第六章：顶会案例 — Alpha-SQL、AFlow、LEAD

本章剖析三个近期顶会工作的写作思路，帮助理解如何把复杂的研究包装成清晰的论文。

## 6.1 Alpha-SQL（ICML'25）

Alpha-SQL 聚焦于文本到 SQL（Text-to-SQL）任务，核心贡献通常包括：

- **问题定义**：在复杂数据库 schema 和多表连接场景下，现有方法容易产生错误查询。
- **方法亮点**：通过某种形式的 agentic 搜索、验证或合成来提升 SQL 生成的可靠性。
- **故事线**：从“现有大模型会写错 SQL”出发，提出“先生成再验证/迭代”的框架，用大量跨领域 benchmark 证明优势。

**可学习的写作策略**：
- 用具体错误案例作为 motivation figure。
- 把方法拆成“生成器 + 验证器 + 搜索/选择器”三个模块，降低理解成本。
- 实验部分覆盖多个公开 benchmark，并给出错误类型分析。

## 6.2 AFlow（ICLR'25）

AFlow 通常指自动化工作流/Agent 工作流优化相关的工作，核心在于：

- **问题定义**：如何自动发现或优化 multi-agent workflow？
- **方法亮点**：把 workflow 设计转化为搜索/优化问题，例如基于 LLM 的演化搜索或强化学习。
- **故事线**：从“手工设计 workflow 费时且难以泛化”出发，提出自动优化框架。

**可学习的写作策略**：
- 用一张总览图展示搜索空间和优化目标。
- 在方法部分先给出形式化定义，再讲算法细节。
- 实验中对比“手工设计 workflow” vs. “自动发现 workflow”。

## 6.3 LEAD（VLDB'26）

LEAD 通常与数据库或数据挖掘领域相关，可能涉及：

- **问题定义**：某个具体的数据库任务（如查询优化、数据清洗、索引选择等）中的新挑战。
- **方法亮点**：结合深度学习与传统数据库技术，提出新的建模或系统方案。
- **故事线**：从真实数据库系统的痛点出发，强调实用性和可部署性。

**可学习的写作策略**：
- 强调方法的系统级影响，例如延迟、吞吐量、存储开销。
- 提供详细的实验设置，包括硬件、数据集规模和 baseline 配置。
- 讨论工程取舍和局限性，增强可信度。

## 6.4 共同特点

- **问题驱动**：每篇论文都从一个具体、可感知的问题开始。
- **模块清晰**：方法被拆成少数几个高内聚、低耦合的组件。
- **实验扎实**：主实验 + 消融 + 案例分析，覆盖多个维度。
- **视觉辅助**：总览图、流程图和关键结果图贯穿全文。

## 6.5 应用到自己的写作

- 在动笔前，明确你的工作和这三个案例中的哪一个更接近。
- 借鉴其章节组织方式，但不要照搬结构。
- 始终回到审稿人视角：读完第一页，对方是否愿意继续读下去？
"#,
        ),
    ]
}
