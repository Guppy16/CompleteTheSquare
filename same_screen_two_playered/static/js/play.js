function move_made(buttObj) {
    var move = buttObj.id;
    var curr_game_name = buttObj.name;
    var data = {"move": move, "curr_game_name": curr_game_name};
    as_json = JSON.stringify(data);
    console.log(as_json);

    // POST
    fetch('/passData', {

        // Specify the method
        method: 'POST',

        // A JSON payload
        body: as_json,
        cache: "no-cache",
        headers: new Headers({
          "content-type": "application/json"
        })
    }).then(function (response) { // At this point, Flask has printed our JSON
        return response.text();
    }).then(function (text) {

        console.log('POST response: ');

        // Should be 'OK' if everything was successful
        console.log(text);
    });
};