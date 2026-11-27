#[path = "../src/state.rs"]
mod state;
#[path = "../src/sanitizer.rs"]
mod sanitizer;
#[path = "../src/userscripts.rs"]
mod userscripts;
#[path = "../src/downloader.rs"]
mod downloader;
#[path = "../src/https_guard.rs"]
mod https_guard;
#[path = "../src/blocker.rs"]
mod blocker;
#[path = "../src/updater.rs"]
mod updater;
#[path = "../src/importer.rs"]
mod importer;
#[path = "../src/passwords.rs"]
mod passwords;
#[path = "../src/little.rs"]
mod little;

use std::panic::{catch_unwind, AssertUnwindSafe};
struct Rng(u64);
impl Rng { fn next(&mut self)->u64{ self.0^=self.0<<13; self.0^=self.0>>7; self.0^=self.0<<17; self.0 }
  fn below(&mut self,n:usize)->usize{ (self.next()%(n.max(1) as u64)) as usize } }
const FRAG:&[&str]=&["http://","https://","//","..","/","\\","%","%00","%2e%2e","\u{202e}","\u{0}","[::1]","[","]","@",":","?","#","&","=",";","\"","'","<",">","{","}","*","^","|","~","é","漢","😀","a","b.com","127.0.0.1","0x7f.1","localhost","xn--","filename*=UTF-8''","attachment; filename=","bytes 0-1/","CON","NUL",".","  ","\t","\n","javascript:","data:","file:///","chrome-extension://","shunno://newtab","10.0.0.1","fe80::1","1e999","-1","18446744073709551616"];
fn rs(r:&mut Rng)->String{ let n=r.below(8); let mut s=String::new(); for _ in 0..n { s.push_str(FRAG[r.below(FRAG.len())]); } s }
fn run<F:Fn(&str)>(name:&str,f:F){ let mut r=Rng(0x9E3779B97F4A7C15); let mut bad=Vec::new();
  for _ in 0..30000 { let s=rs(&mut r); if catch_unwind(AssertUnwindSafe(||f(&s))).is_err(){ bad.push(s); if bad.len()>=3{break;} } }
  assert!(bad.is_empty(), "{name} panicked on {:?}", bad); }

#[test] fn fuzz_text_parsers(){
  run("extract_host",|s|{ let _=sanitizer::extract_host(s); });
  run("is_public_http_url",|s|{ let _=sanitizer::is_public_http_url(s); });
  run("is_public_domain",|s|{ let _=sanitizer::is_public_domain(s); });
  run("is_blocked_ad_url",|s|{ let _=sanitizer::is_blocked_ad_url(s); });
  run("sanitize_url",|s|{ let _=sanitizer::sanitize_url(s,&state::GlobalSettings::default()); });
  run("search_url",|s|{ let _=sanitizer::search_url(s,&state::GlobalSettings::default()); });
  run("selection_query",|s|{ let _=sanitizer::selection_query(s); });
  run("sanitize_download_filename",|s|{ let n=downloader::sanitize_download_filename(s); assert!(!n.contains('/')&&!n.contains('\\')&&!n.contains(':')&&!n.is_empty(), "bad name {:?} from {:?}", n, s); });
  run("content_disposition",|s|{ let _=downloader::parse_content_disposition_filename(s); });
  run("content_range",|s|{ let _=downloader::parse_content_range(s); });
  run("split_route_prefix",|s|{ let _=downloader::split_route_prefix(s); });
  run("is_ytdlp/hls",|s|{ let _=downloader::is_ytdlp_url(s); let _=downloader::is_hls_url(s); });
  run("netscape_cookie_jar",|s|{ let _=downloader::netscape_cookie_jar("a.com",s); let _=downloader::netscape_cookie_jar(s,"a=b; c=d"); });
  run("clean_group",|s|{ let _=state::clean_group(s,s); });
  run("reader_prefs",|s|{ let _=state::valid_reader_prefs(s); });
  run("site_policy_for",|s|{ let _=state::site_policy_for(s); });
  run("updater version",|s|{ let _=updater::parse_version(s); let _=updater::is_newer(s,"2.0.0"); });
  run("bookmarks json",|s|{ let _=importer::parse_chromium_bookmarks(s,"x"); });
  run("blocker rules",|s|{ let _=blocker::invalid_rules(s); let _=blocker::clean_user_text(s); let _=blocker::looks_like_list(s); let _=blocker::picker_rule(s,s); });
  run("hls",|s|{ let b=url::Url::parse("https://a.com/x/y.m3u8").unwrap(); let _=downloader::parse_hls_playlist(s,&b); });
  run("https_guard html",|s|{ let _=https_guard::interstitial_html(s,s); });
}

#[test] fn fuzz_binary_parsers(){
  let mut r=Rng(0xC0FFEE123456789);
  // Random bytes and mutated SQLite-like headers.
  let mut seed=b"SQLite format 3\0".to_vec(); seed.resize(4096,0); seed[16]=0x10; seed[17]=0; seed[28]=0;seed[31]=1; seed[100]=0x0d;
  for k in 0..20000 {
    let mut db:Vec<u8>= if k%2==0 { (0..r.below(6000)).map(|_|r.next() as u8).collect() } else { seed.clone() };
    if k%2==1 { for _ in 0..r.below(40) { let i=r.below(db.len()); db[i]=r.next() as u8; } if r.below(4)==0 { let n=r.below(db.len()); db.truncate(n); } }
    let wal:Vec<u8>=(0..r.below(3000)).map(|_|r.next() as u8).collect();
    let res=catch_unwind(AssertUnwindSafe(||{ let _=importer::parse_chromium_history(&db,100); let _=importer::apply_wal(&db,&wal); }));
    assert!(res.is_ok(), "binary parser panicked on case {k}: db len {} head {:?}", db.len(), &db[..db.len().min(32)]);
  }
}
