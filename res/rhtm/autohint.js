$(document).ready(function() {
	var wordhistory=[];
	var curhistoryidx=0;
	var flag = false;
	var dict_content = $("#dict_content");
	var qword = $("#qwt");
	var formobj = $("#qwFORM");
	var chkreg = document.getElementById("chkreg");
	qword.autocomplete({
		//autoFocus:true,
		source:function(req, res) {
			$.ajax({
				url:"/n/" + req.term,
				type:"GET",
				dataType:"text",
				success:function(data) {
					res(data.split("\n"));
				},
				error:function() {
					res(["ERROR"]);
				}
			});
		},
		close:function(e,ui) {
			if (flag)
				formobj.submit();
			flag = false;
		},
		select:function(e,ui) {
			flag = true;
		}
	});
	function loadcontent(cnt) {
		dict_content.html(cnt.replace(/<a>([^<]*)<\/a>/g, '<a href="/w/$1">$1</a>'));
		$("a").click(function(e) {
			if (this.href.length == 0) {
				e.preventDefault();
				qword.val(decodeURI(this.innerHTML));
				formobj.submit();
			} else if (this.href.indexOf("/w/") >= 0) {
				e.preventDefault();
				var targetword = this.href.replace(/^.*\/w\/([^&]+).*$/, "$1");
				qword.val(decodeURI(targetword));
				formobj.submit();
			}
		});
		window.scrollTo(0,0);
		if (curhistoryidx == 0 || wordhistory[curhistoryidx - 1] != qword.val()) {
			wordhistory[curhistoryidx] = qword.val();
			if (wordhistory.length > 30) {
				wordhistory.shift();
			} else {
				++curhistoryidx;
			}
		}
	}
	formobj.on("submit", function(e) {
		e.preventDefault();
		var lookup = "/W/";
		if (chkreg.checked) {
			lookup = "/s/";
			chkreg.checked = false;
		}
		$.ajax({
			url:lookup+qword.val(),
			type:"GET",
			dataType:"html",
			success:function(data) {
				loadcontent(data);
			},
			error:function(d,txt) {
				loadcontent(txt);
			}
		});
		return false;
	});
	$("#backwardbtn").on("click", function() {
		if (curhistoryidx < 0) {
			curhistoryidx = 0;
			return;
		}
		if (curhistoryidx > 1) {
			curhistoryidx -= 2;
			qword.val(wordhistory[curhistoryidx]);
			formobj.submit();
		}
	});
	$("#forwardbtn").on("click", function() {
		if (curhistoryidx >= 0 && curhistoryidx < wordhistory.length) {
			qword.val(wordhistory[curhistoryidx]);
			formobj.submit();
		} else {
			curhistoryidx = wordhistory.length;
		}
	});
	if (window.location.href.match(/w\/..*/)) {
		var w = window.location.href.replace(/.*\/w\//, "");
		qword.val(decodeURI(w));
		formobj.submit();
	}
});
