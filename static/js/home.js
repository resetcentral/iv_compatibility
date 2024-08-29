function updateIvCount() {
  let ivsInput = $("#num-ivs")[0];
  if (ivsInput.value < ivsInput.min) {
    ivsInput.value = 1;
  }
  let num = ivsInput.value;
  
  let ivs = $(".iv");
  let lastIv = ivs.last();
  if (ivs.length < num) {
    let firstIv = ivs.first();
    for (let i = ivs.length; i < num; i += 1) {
      let newIv = firstIv.clone();
      let title = newIv.find(".iv-title")
      title.text("IV #" + (i+1));
      let ivInputs = newIv.find(".infusion-input select");
      let select = ivInputs.first();
      select.attr("name", "iv-" + i);
      select.find("option")[0].selected = true;
      ivInputs.slice(1).remove();

      lastIv.after(newIv);
      lastIv = newIv;
    }
  } else if (ivs.length > num) {
    ivs.slice(num).remove();
  }
}

function deleteInfusion(event) {
  let thisInput = $(event.currentTarget.parentElement);
  let allInputs = thisInput.parent().find(".infusion-input");
  
  if (allInputs.length == 1) {
    let selector = thisInput.parent().find(".infusion-input-dropdown");
    selector.find("option")[0].selected = true;
  } else {
    thisInput.remove();
  }
}

function addInfusion(event) {
  let newInput = $(event.currentTarget.parentElement).find(".infusion-input").first().clone();
  newInput.find(".infusion-input-dropdown").find("option")[0].selected = true;

  $(event.currentTarget).before(newInput);
}

function submitData() {
  let data = new FormData($("#input-form")[0]);
  let ivs = [];
  for (const i of Array(data.get("num-ivs")).keys()) {
    ivs.push(Array.from(data.getAll("iv-" + i)));
  }

  ivs = JSON.stringify(ivs);

  let parsedData = new FormData();
  parsedData.append("num_ivs", data.get("num-ivs"));
  parsedData.append("ivs", ivs);
  parsedData.append("add", data.getAll("add"));
  console.log(parsedData);

  queryString = new URLSearchParams(parsedData).toString();
  window.open("/results?" + queryString)
}