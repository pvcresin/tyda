#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "integration test assertions"
)]

//! Missing-method diagnostics on schema-backed Active Record models. A model's class
//! surface and gem-generated methods are only partly modeled, so those calls stay silent
//! while a missing instance method on a known schema is still reported.

use std::collections::BTreeSet;
use std::process::Command;

use tempfile::tempdir;

const SCHEMA: &str = r#"
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "reports", force: :cascade do |t|
    t.string "title"
    t.string "file"
    t.string "encrypted_token"
  end
  create_table "billing_invoices", force: :cascade do |t|
    t.string "number"
  end
  create_table "licenses", force: :cascade do |t|
    t.string "plan"
  end
end
"#;

const APPLICATION_RECORD: &str =
    "class ApplicationRecord < ActiveRecord::Base\n  self.abstract_class = true\nend\n";

/// Runs `tyda --diagnostics` on a Rails project and returns the unresolved `Owner#method` set.
fn missing_methods(files: &[(&str, &str)]) -> BTreeSet<String> {
    let project = tempdir().expect("temporary project");
    let root = project.path();
    let mut all_files = vec![
        (
            "Gemfile",
            "source \"https://rubygems.org\"\ngem \"rails\"\ngem \"devise\"\n",
        ),
        ("db/schema.rb", SCHEMA),
        ("app/models/application_record.rb", APPLICATION_RECORD),
    ];
    all_files.extend_from_slice(files);
    for (path, content) in all_files {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().expect("parent directory"))
            .expect("create directory");
        std::fs::write(path, content).expect("write project file");
    }

    let output = Command::new(env!("CARGO_BIN_EXE_tyda"))
        .args(["--diagnostics", "app"])
        .current_dir(root)
        .output()
        .expect("run Tyda diagnostics");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("UTF-8 diagnostics")
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|diagnostic| diagnostic["code"] == "missing_method")
        .filter_map(|diagnostic| diagnostic["unresolved_method"].as_str().map(str::to_string))
        .collect()
}

#[test]
fn model_class_methods_from_rails_and_gems_are_not_reported() {
    let missing = missing_methods(&[
        (
            "app/models/billing.rb",
            "module Billing\n  def self.table_name_prefix = \"billing_\"\nend\n",
        ),
        (
            "app/models/billing/invoice.rb",
            concat!(
                "class Billing::Invoice < ApplicationRecord\n",
                "  scope :numbered, -> { where(\"#{table_name}.number IS NOT NULL\") }\n",
                "  state_machine :status do\n",
                "    state :open\n",
                "  end\n",
                "  def self.record(number)\n",
                "    upsert({ number: number }, unique_by: :number)\n",
                "  end\n",
                "  def label = Billing::Invoice.human_attribute_name(:number)\n",
                "  def broken = numbr\n",
                "end\n",
            ),
        ),
    ]);
    assert_eq!(
        missing,
        BTreeSet::from(["Billing::Invoice#numbr".to_string()]),
        "{missing:?}"
    );
}

#[test]
fn class_methods_of_a_concern_follow_the_model_class_surface() {
    let missing = missing_methods(&[
        (
            "app/models/concerns/quoted_columns.rb",
            concat!(
                "module QuotedColumns\n",
                "  extend ActiveSupport::Concern\n",
                "  class_methods do\n",
                "    def quoted_title = adapter_class.quote_column_name(:title)\n",
                "  end\n",
                "end\n",
            ),
        ),
        (
            "app/models/report.rb",
            "class Report < ApplicationRecord\n  include QuotedColumns\nend\n",
        ),
    ]);
    assert!(missing.is_empty(), "{missing:?}");
}

#[test]
fn names_given_to_unknown_macros_are_not_reported() {
    let missing = missing_methods(&[(
        "app/models/report.rb",
        concat!(
            "class Report < ApplicationRecord\n",
            "  attr_encrypted :token, key: \"secret\"\n",
            "  mount_uploader :file, ReportUploader\n",
            "  def rotate?\n",
            "    token.present? && token_changed?\n",
            "  end\n",
            "  def content = file.read\n",
            "  def drop = remove_file!\n",
            "  def cell(name) = self[\"cell_#{name}\"]\n",
            "  def summary = slice(:title)\n",
            "  def broken = secret_value\n",
            "end\n",
        ),
    )]);
    assert_eq!(
        missing,
        BTreeSet::from(["Report#secret_value".to_string()]),
        "{missing:?}"
    );
}

#[test]
fn modules_included_at_runtime_open_the_model_surface() {
    let missing = missing_methods(&[
        (
            "app/models/concerns/routing.rb",
            concat!(
                "module Routing\n",
                "  extend ActiveSupport::Concern\n",
                "  included do\n",
                "    include Routing.url_helpers\n",
                "  end\n",
                "  def self.url_helpers = Rails.application.routes.url_helpers\n",
                "end\n",
            ),
        ),
        (
            "app/models/report.rb",
            concat!(
                "class Report < ApplicationRecord\n",
                "  include Routing\n",
                "  def url = report_download_url(self)\n",
                "end\n",
            ),
        ),
        (
            "app/models/user.rb",
            concat!(
                "class User < ApplicationRecord\n",
                "  devise :database_authenticatable, :recoverable\n",
                "  def expired? = !reset_password_period_valid?\n",
                "end\n",
            ),
        ),
    ]);
    assert!(missing.is_empty(), "{missing:?}");
}

#[test]
fn a_table_without_a_model_class_does_not_prove_its_surface() {
    let missing = missing_methods(&[(
        "app/models/report.rb",
        concat!(
            "class Report < ApplicationRecord\n",
            "  def licensed? = License.feature_available?(:reports)\n",
            "end\n",
        ),
    )]);
    assert!(missing.is_empty(), "{missing:?}");
}

#[test]
fn readers_generated_from_macro_names_and_select_aliases_are_not_reported() {
    let missing = missing_methods(&[
        (
            "app/models/concerns/file_mounter.rb",
            concat!(
                "module FileMounter\n",
                "  extend ActiveSupport::Concern\n",
                "  class_methods do\n",
                "    def mount_file(uploader, field: :file)\n",
                "      mount_uploader(field, uploader)\n",
                "    end\n",
                "  end\n",
                "end\n",
            ),
        ),
        (
            "app/models/report.rb",
            concat!(
                "class Report < ApplicationRecord\n",
                "  include FileMounter\n",
                "  mount_file ReportUploader\n",
                "  attribute :draft_path, default: nil\n",
                "  jsonb_accessor :settings, level: :integer\n",
                "  def readable? = file.exists? && draft_path?\n",
                "  def level_changed_recently? = level_changed?\n",
                "  def summary = self.class.where(title: \"x\").select(\"count(*) AS total\").first!.total\n",
                "  def broken = summary_text\n",
                "end\n",
            ),
        ),
        (
            "app/workers/report_worker.rb",
            "class ReportWorker\n  def perform(id) = Report.find(id).file.open_stream\nend\n",
        ),
    ]);
    assert_eq!(
        missing,
        BTreeSet::from(["Report#summary_text".to_string()]),
        "{missing:?}"
    );
}
