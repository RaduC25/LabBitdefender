#[macro_use] extern crate rocket;

use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader,BufWriter},
    path::{Path, PathBuf},
    time::Instant,
    sync::{Arc, RwLock},
};

use clap::Parser;

use rocket::State;

use serde::{Deserialize, Serialize};

use rocket::{fs::FileServer, serde::json::Json};

type Term = String;
type DocumentId = String;
#[derive(Default,Serialize,Deserialize)]
struct TermData {
    term_docs: HashMap<DocumentId, u64>,
    idf: f64,
}
#[derive(Default,Serialize,Deserialize)]
struct IndexedData {
    term_data: HashMap<Term, TermData>,
    doc_len: HashMap<DocumentId, u64>,
    avgdl: f64,
}

impl IndexedData {
    fn new() -> Self {
        Self {
            term_data: HashMap::new(),
            doc_len: HashMap::new(),
            avgdl: 0.0,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct FileData {
    /// name of the zip archive
    name: DocumentId,
    /// list of files in the zip archive
    files: Vec<String>,
}

const K1: f64 = 1.6;
const B: f64 = 0.75;





fn load_data(data_filename: impl AsRef<Path>, _limit: Option<usize>) -> eyre::Result<IndexedData> {
    let data_filename = data_filename.as_ref();
    let file = File::open(data_filename)?;
    let reader = BufReader::new(file);

    let mut index = IndexedData::new();

    let mut n: i32 = 0;

    for line in reader.lines() {
        n += 1;
        let line = line?;

        let fd: FileData = serde_json::from_str(&line)?;
        let mut doc_len = 0;
        for file in fd.files {
            for term in file.split("/") {
                doc_len += 1;
               
                if let Some(td) = index.term_data.get_mut(term) {
                    
                    let x = td.term_docs.entry(fd.name.clone()).or_insert(0);
                    *x += 1;

                } else {
                    let mut map = HashMap::new();
                    map.insert(fd.name.clone(), 1);
                    let td = TermData {
                        term_docs: map,
                        idf: 0.0,
                    };
                    index.term_data.insert(term.to_string(), td);
                }
            }
        }
        index.doc_len.insert(fd.name, doc_len);
        index.avgdl += doc_len as f64;
    }
    index.avgdl /= n as f64;

    for (_term, entries) in index.term_data.iter_mut() {
        let nq = entries.term_docs.len() as f64;
        entries.idf = (((n as f64 - nq + 0.5) / (nq + 0.5)) + 1.0).ln();
    }

    Ok(index)
}



fn run_search(data: &IndexedData, terms: Vec<String>) -> Vec<(DocumentId, f64)> {
    let mut counter: HashMap<DocumentId, f64> = HashMap::new();
    for term in &terms {
        if let Some(td) = data.term_data.get(term) {
            for (doc,app)in td.term_docs.iter() {
                let x: &mut f64 = counter.entry(doc.to_string()).or_insert(0.0);
                *x += td.idf * (*app as f64 * (K1 + 1.0))
                    / (*app as f64 + K1 * (1.0 - B + B * data.doc_len[doc] as f64 / data.avgdl));
            }
        }
    }

    let mut scores: Vec<(DocumentId, f64)> = Vec::new();
    for (doc, cnt) in counter {
        scores.push((doc.to_string(), cnt  / terms.len() as f64));
    }
    scores.sort_by(|a, b| b.1.total_cmp(&a.1));
    scores
}


#[derive(Serialize)]
struct Greeting {
    message: String,
}


#[derive(Default)]
struct ServerState {
    index: IndexedData,
}

#[derive(Deserialize)]
struct SearchData{
    terms: Vec<String>,
    max_length: Option<usize>,
    min_score: Option<f64>,
}

#[derive(Serialize)]
struct SearchResult{
    matches: Vec<SearchMatch>,
}

#[derive(Serialize)]
struct SearchMatch {
    md5: DocumentId,
    score: f64,
}

#[get("/")]
fn index() -> Json<Greeting> {
    Json(Greeting {
        message: "Hello, welcome to our server!".to_string(),
    })
}


use rocket::fs::TempFile;

#[derive(FromForm)]

struct Upload<'r>{
    file:TempFile<'r>,
    max_length: Option<usize>,
    min_score: Option<f64>,
}
 
#[post("/search_by_file",data="<upload>")]
fn search_by_file(upload: rocket::form::Form<Upload<'_>>){
    let file = File::open(upload.file.path().unwrap()).unwrap();
    let reader = BufReader::new(file);
    let mut zip =zip::ZipArchive::new(reader).unwrap();
    let mut filenames =Vec::new();

    for i in 0..zip.len(){
        let _file = zip.by_index(i).unwrap();
        filenames.push(_file.name().to_string());
    }

    let mut filename_parts = Vec::new();
    for filename in &filenames{
        filename_parts.extend(filename.split("/"));
    }
}



#[post("/search", data= "<req>")]


fn search(req: Json<SearchData>, server_state: &State<Arc<RwLock<ServerState>>>) -> Result<Json<SearchResult>, String> {
    let terms = req.terms.clone();
    let min_score = req.min_score.unwrap_or(0.0);
    let server_state = server_state.read().map_err(|err| format!("Error: {err:#}"))?;
    let matches = run_search(&server_state.index, terms)
        .into_iter()
        .map(|(md5, score)| SearchMatch { md5, score })
        .filter(|x| x.score >= min_score)
        .take(req.max_length.unwrap_or(usize::MAX))
        .collect();
    Ok(Json(SearchResult { matches }))
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long)]
    build_from: Option<PathBuf>,

    #[arg(long)]
    load_from: Option<PathBuf>,

    #[arg(long)]
    save_to: Option<PathBuf>,

    #[arg(long)]
    limit: Option<usize>,
}


#[rocket::main]
async fn main() -> eyre::Result<()> {
    
    let args = Args::parse();
    //let data_filename = &args[1];

    let start = Instant::now();
    //let data = load_data(data_filename)?;
    let data = if let Some(input) = &args.build_from {
        load_data(input, args.limit)?
    } else if let Some(saved) = &args.load_from {
        let file = File::open(saved)?;
        let reader = BufReader::new(file);
        rmp_serde::from_read(reader)?
    } else {
        eprintln!("either input or saved data must be provided");
        std::process::exit(1);
    };
    println!("loaded data for {} terms", data.term_data.len());
    println!("elapsed time: {:?}", start.elapsed());

    let pair_count = data
        .term_data.values().map(|td| td.term_docs.len()).sum::<usize>();
    println!("there are {} term-docid pairs", pair_count);

    let start = Instant::now();
    let search: Vec<String> = vec!["lombok".to_string(), "AUTHORS".to_string(), "README.md".to_string()];
    run_search(&data, search);
    println!("search took: {:?}", start.elapsed());

    if let Some(save_to) = &args.save_to {
        let start = Instant::now();
        let file = File::create(save_to)?;
        let mut file = BufWriter::new(file);
        rmp_serde::encode::write_named(&mut file, &data)?;
        println!("saved data in {:.2}s", start.elapsed().as_secs_f64());
    }



    let server_state = Arc::new(RwLock::new(ServerState {
        index: data,
    }));
    rocket::build()
    .manage(server_state)
    .mount("/", routes![index,search])
    .mount("/dashboard", FileServer::from("static"))
    .ignite().await?
    .launch().await?;
    Ok(())
}